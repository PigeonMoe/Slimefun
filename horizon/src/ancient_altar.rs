// SPDX-License-Identifier: GPL-3.0-only

use std::sync::{
    OnceLock,
    atomic::{AtomicI32, Ordering},
};

use mod_api::{
    AttachmentCodec, AttachmentType, BlockDropsEvent, Compound, EntityUuid, GameMode, Hand, Holder,
    Host, HostSide, HostTransaction, ITEMS, InteractionResult, ItemDrop, ItemPart, ItemPlace,
    ItemResource, ItemStack, LevelHost, LevelRef, OnBlockPlacedByEvent, ParticleKey,
    ParticleOptions, PlayerRef, Registrar, ServerHost, SoundSource, Tag, Text, UseOnEvent,
};

use crate::{
    backpacks::{ESSENCE_OF_AFTERLIFE, NECROMANCY, NECROTIC_SKULL},
    items::{ENDER_LUMP_3, GOLD_8K, Kind, MAGIC_LUMP_1, MAGIC_LUMP_2, MAGIC_LUMP_3, item, text},
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

pub(crate) const ALTAR_RESEARCH: Research = Research {
    id: "ancient_altar",
    name: "Ancient Altar",
    cost: 15,
};
pub(crate) const RUNES_RESEARCH: Research = Research {
    id: "ancient_runes",
    name: "Elemental Runes",
    cost: 15,
};

item!(
    ANCIENT_ALTAR,
    "enchanting_table",
    "Ancient Altar",
    "light_purple",
    None,
    None
);
item!(
    ANCIENT_PEDESTAL,
    "dispenser",
    "Ancient Pedestal",
    "light_purple",
    None,
    None
);
item!(
    BLANK_RUNE,
    "firework_star",
    "Blank Rune",
    "dark_gray",
    None,
    None
);
item!(
    ANCIENT_RUNE_AIR,
    "firework_star",
    "Ancient Rune [Air]",
    "gray",
    None,
    None
);
item!(
    ANCIENT_RUNE_EARTH,
    "firework_star",
    "Ancient Rune [Earth]",
    "gray",
    None,
    None
);
item!(
    ANCIENT_RUNE_FIRE,
    "firework_star",
    "Ancient Rune [Fire]",
    "gray",
    None,
    None
);
item!(
    ANCIENT_RUNE_WATER,
    "firework_star",
    "Ancient Rune [Water]",
    "gray",
    None,
    None
);

macro_rules! vanilla {
    ($id:literal) => {
        Kind::Vanilla(concat!("minecraft:", $id))
    };
}

pub(crate) const MAGIC_CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            Some(vanilla!("obsidian")),
            Some(GOLD_8K),
            Some(vanilla!("obsidian")),
            None,
            Some(vanilla!("stone")),
            None,
            Some(vanilla!("obsidian")),
            Some(GOLD_8K),
            Some(vanilla!("obsidian")),
        ],
        output: Stack {
            kind: ANCIENT_PEDESTAL,
            count: 4,
        },
        research: &ALTAR_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(vanilla!("enchanting_table")),
            None,
            Some(MAGIC_LUMP_3),
            Some(GOLD_8K),
            Some(MAGIC_LUMP_3),
            Some(vanilla!("obsidian")),
            Some(GOLD_8K),
            Some(vanilla!("obsidian")),
        ],
        output: Stack {
            kind: ANCIENT_ALTAR,
            count: 1,
        },
        research: &ALTAR_RESEARCH,
    },
];

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    altar_recipe(
        [
            vanilla!("stone"),
            MAGIC_LUMP_1,
            vanilla!("stone"),
            MAGIC_LUMP_1,
            vanilla!("obsidian"),
            MAGIC_LUMP_1,
            vanilla!("stone"),
            MAGIC_LUMP_1,
            vanilla!("stone"),
        ],
        BLANK_RUNE,
        1,
        &RUNES_RESEARCH,
    ),
    altar_recipe(
        [
            vanilla!("feather"),
            MAGIC_LUMP_1,
            vanilla!("feather"),
            vanilla!("ghast_tear"),
            BLANK_RUNE,
            vanilla!("ghast_tear"),
            vanilla!("feather"),
            MAGIC_LUMP_1,
            vanilla!("feather"),
        ],
        ANCIENT_RUNE_AIR,
        4,
        &RUNES_RESEARCH,
    ),
    altar_recipe(
        [
            vanilla!("dirt"),
            MAGIC_LUMP_1,
            vanilla!("stone"),
            vanilla!("obsidian"),
            BLANK_RUNE,
            vanilla!("obsidian"),
            vanilla!("stone"),
            MAGIC_LUMP_1,
            vanilla!("dirt"),
        ],
        ANCIENT_RUNE_EARTH,
        4,
        &RUNES_RESEARCH,
    ),
    altar_recipe(
        [
            vanilla!("fire_charge"),
            MAGIC_LUMP_2,
            vanilla!("fire_charge"),
            vanilla!("blaze_powder"),
            ANCIENT_RUNE_EARTH,
            vanilla!("flint_and_steel"),
            vanilla!("fire_charge"),
            MAGIC_LUMP_2,
            vanilla!("fire_charge"),
        ],
        ANCIENT_RUNE_FIRE,
        4,
        &RUNES_RESEARCH,
    ),
    altar_recipe(
        [
            vanilla!("salmon"),
            MAGIC_LUMP_2,
            vanilla!("water_bucket"),
            vanilla!("sand"),
            BLANK_RUNE,
            vanilla!("sand"),
            vanilla!("water_bucket"),
            MAGIC_LUMP_2,
            vanilla!("cod"),
        ],
        ANCIENT_RUNE_WATER,
        4,
        &RUNES_RESEARCH,
    ),
    altar_recipe(
        [
            ENDER_LUMP_3,
            ANCIENT_RUNE_AIR,
            ENDER_LUMP_3,
            ANCIENT_RUNE_EARTH,
            NECROTIC_SKULL,
            ANCIENT_RUNE_FIRE,
            ENDER_LUMP_3,
            ANCIENT_RUNE_WATER,
            ENDER_LUMP_3,
        ],
        ESSENCE_OF_AFTERLIFE,
        1,
        &NECROMANCY,
    ),
    crate::infused::ALTAR,
];

const fn altar_recipe(
    [a, b, c, d, e, f, g, h, i]: [Kind; 9],
    kind: Kind,
    count: u32,
    research: &'static Research,
) -> CraftingRecipe {
    CraftingRecipe {
        inputs: [
            Some(a),
            Some(b),
            Some(c),
            Some(d),
            Some(e),
            Some(f),
            Some(g),
            Some(h),
            Some(i),
        ],
        output: Stack { kind, count },
        research,
    }
}

const PEDESTAL_PATTERN: [usize; 8] = [0, 1, 2, 5, 8, 7, 6, 3];
const PEDESTAL_OFFSETS: [[i32; 2]; 8] = [
    [2, -2],
    [3, 0],
    [2, 2],
    [0, 3],
    [-2, 2],
    [-3, 0],
    [-2, -2],
    [0, -3],
];

const DEVICE: AttachmentType<Compound> = attachment("ancient_altar_device");
const RITUALS: AttachmentType<Compound> = attachment("ancient_altar_rituals");
const DISPLAYS: AttachmentType<Compound> = attachment("ancient_pedestal_displays");

const fn attachment(name: &'static str) -> AttachmentType<Compound> {
    AttachmentType {
        namespace: "slimefun",
        name,
        default: Compound::new,
        codec: Some(AttachmentCodec {
            encode: |value| Ok(value.clone()),
            decode: |value| Ok(value.clone()),
        }),
        copy_on_death: false,
        sync: false,
    }
}

type Pos = [i32; 3];
const START_DELAY: i64 = 10;
const DEFAULT_STEP_DELAY: i32 = 8;
const FINISH_STAGE: i32 = 36;
static STEP_DELAY: AtomicI32 = AtomicI32::new(DEFAULT_STEP_DELAY);
static RITUAL_PARTICLES: OnceLock<[ParticleKey; 4]> = OnceLock::new();

pub(crate) fn configure(delay: Option<i32>) {
    STEP_DELAY.store(delay.unwrap_or(DEFAULT_STEP_DELAY), Ordering::Relaxed);
}

fn pedestals([x, y, z]: Pos) -> Option<[Pos; 8]> {
    let mut result = [[0; 3]; 8];
    for (block, [dx, dz]) in result.iter_mut().zip(PEDESTAL_OFFSETS) {
        *block = [x.checked_add(dx)?, y, z.checked_add(dz)?];
    }
    Some(result)
}

fn id(data: &Compound) -> Option<Kind> {
    let Tag::String(id) = data.get(&"id".into())? else {
        return None;
    };
    let id = id.to_string().ok()?;
    paper_device(&id)
}

pub(crate) fn paper_device(id: &str) -> Option<Kind> {
    [ANCIENT_ALTAR, ANCIENT_PEDESTAL]
        .into_iter()
        .find(|kind| kind.id() == id)
}

fn legacy_probe_text(text: &str) -> bool {
    let mut characters = text.chars();
    let mut plain = String::new();
    while let Some(character) = characters.next() {
        if character == '§' {
            characters.next();
        } else {
            plain.push(character);
        }
        if plain.len() > 64 {
            break;
        }
    }
    plain.starts_with("ALTAR Probe - ")
}

fn probe_name(name: &Tag, prefix: &mut String, depth: u8) {
    if depth == 16 || prefix.len() >= 64 {
        return;
    }
    match name {
        Tag::String(text) => {
            for character in text.to_string_lossy().chars().take(64) {
                prefix.push(character);
            }
        }
        Tag::Compound(text) => {
            if let Some(text) = text.get(&"text".into()) {
                probe_name(text, prefix, depth + 1);
            }
            if let Some(extra) = text.get(&"extra".into()) {
                probe_name(extra, prefix, depth + 1);
            }
        }
        Tag::List(parts) => {
            for part in parts {
                if prefix.len() >= 64 {
                    break;
                }
                probe_name(part, prefix, depth + 1);
            }
        }
        _ => {}
    }
}

fn has_paper_display(host: &dyn Host, level: LevelRef, pos: Pos) -> Option<bool> {
    let [x, y, z] = pos.map(f64::from);
    let candidates = level.entities_in(host, [x, y + 0.7, z], [x + 1.0, y + 1.7, z + 1.0])?;
    Some(candidates.into_iter().any(|entity| {
        if entity
            .name(host)
            .is_some_and(|name| legacy_probe_text(&name))
        {
            return true;
        }
        let Some(item) = host.item_entity(entity) else {
            return false;
        };
        if item.pickup_delay(host) == Some(i32::from(i16::MAX)) {
            return true;
        }
        let Some(stack) = item.stack(host) else {
            return false;
        };
        let Some(name) = stack
            .components()
            .and_then(|data| data.as_nbt().get(&"minecraft:custom_name".into()))
        else {
            return false;
        };
        let mut prefix = String::new();
        probe_name(name, &mut prefix, 0);
        legacy_probe_text(&prefix)
    }))
}

fn paper_ingredient(
    host: &dyn ServerHost,
    kind: Kind,
    row: &toml::Table,
) -> Result<Option<ItemStack>, String> {
    if row
        .get("metadata")
        .and_then(toml::Value::as_table)
        .is_some_and(|metadata| metadata.contains_key("item"))
    {
        return Err("Paper does not specify an ItemStack encoding for the altar metadata item field. The original metadata remains archived.".into());
    }
    if !row.contains_key("paper_inventory") {
        return Ok(None);
    }
    crate::storage::validate_inventory_preset(row, kind.id())?;
    let items = crate::storage::decode_archived_slots(host, row, 1)?;
    match items.into_values().next() {
        Some(item) if kind == ANCIENT_PEDESTAL && item.count() == 1 => Ok(Some(item)),
        Some(_) => Err("An Ancient Pedestal archive can contain only one item. The original inventory remains archived.".into()),
        None => Ok(None),
    }
}

pub(crate) fn restore_paper(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Pos,
    kind: Kind,
    row: &toml::Table,
) -> Result<String, String> {
    if !level.in_place(host)
        || !level
            .block_state(host, pos)
            .is_some_and(|state| host.block_key(kind.material()) == Some(state.block()))
    {
        return Err(format!(
            "Load the original {} carrier in the active dimension first.",
            kind.id()
        ));
    }
    let entity = host
        .block_entity(level, pos)
        .ok_or("The Paper altar carrier has no loaded block entity.")?;
    let holder = Holder::BlockEntity(pos);
    if let Some(saved) = DEVICE
        .get_existing(host, holder)
        .map_err(|_| "Could not read the loaded altar marker.")?
    {
        return match id(&saved) {
            Some(saved_kind) if saved_kind == kind => {
                Ok(format!("{} is already restored.", kind.id()))
            }
            _ => Err("This carrier already has a different or damaged altar marker.".into()),
        };
    }
    if host
        .block_entity_saved_attachment(entity, "slimefun", "network_state")
        .is_some()
    {
        return Err("This carrier already belongs to a different Slimefun device.".into());
    }
    let item = paper_ingredient(host, kind, row)?;
    if kind == ANCIENT_PEDESTAL {
        if has_paper_display(host, level, pos) != Some(false) {
            return Err("A Paper pedestal display may still be in the world. Recover its item in Paper before restoring this marker. The original archive and entities remain in place.".into());
        }
        let storage = level
            .capability(host, &ITEMS, pos, None)
            .ok_or("The Paper pedestal has no loaded dispenser inventory.")?;
        if (0..storage.size(host)).any(|slot| storage.amount(host, slot) != 0) {
            return Err("The pedestal carrier's vanilla dispenser still contains items.".into());
        }
    }
    let mut marker = Compound::from([("id".into(), Tag::String(kind.id().into()))]);
    let has_item = item.is_some();
    if let Some(item) = item {
        let saved = item
            .to_nbt(host)
            .ok_or("Could not encode the original pedestal item.")?;
        marker.insert("item".into(), Tag::Compound(saved));
    }
    host.transaction(|host, _| {
        DEVICE
            .set(host, holder, marker)
            .map_err(|_| "Could not save the Paper altar marker.")?;
        if has_item {
            watch_display(host, pos)?;
        }
        Ok::<_, &'static str>(())
    })?;
    Ok(format!("Restored the Paper {}.", kind.id()))
}

fn device(host: &mut dyn ServerHost, level: LevelRef, pos: Pos) -> Option<(Kind, Compound)> {
    host.block_entity(level, pos)?;
    let data = DEVICE.get_existing(host, Holder::BlockEntity(pos)).ok()??;
    let kind = id(&data)?;
    let block = level.block_state(host, pos)?.block();
    (host.block_key(kind.material()) == Some(block)).then_some((kind, data))
}

fn ingredient(host: &dyn Host, pedestal: &Compound) -> Option<ItemStack> {
    let Tag::Compound(nbt) = pedestal.get(&"item".into())? else {
        return None;
    };
    ItemStack::from_nbt(host, nbt).filter(|stack| stack.count() == 1)
}

pub(crate) fn marked(host: &dyn Host, entity: mod_api::BlockEntityRef) -> bool {
    host.block_entity_saved_attachment(entity, DEVICE.namespace, DEVICE.name)
        .is_some()
}

pub(crate) fn is_pedestal(host: &dyn Host, level: LevelRef, pos: Pos) -> bool {
    if !level
        .block_state(host, pos)
        .is_some_and(|state| host.block_key(ANCIENT_PEDESTAL.material()) == Some(state.block()))
    {
        return false;
    }
    host.block_entity_identity(level, pos)
        .and_then(|entity| {
            host.block_entity_saved_attachment(entity, DEVICE.namespace, DEVICE.name)
        })
        .as_ref()
        .and_then(id)
        == Some(ANCIENT_PEDESTAL)
}

pub(crate) fn denies_placement(host: &dyn Host, level: LevelRef, pos: mod_api::BlockPos) -> bool {
    pos.y
        .checked_sub(1)
        .is_some_and(|below| is_pedestal(host, level, [pos.x, below, pos.z]))
}

pub(crate) fn register(registrar: &mut dyn Registrar) {
    let _ = RITUAL_PARTICLES.set(
        [
            "minecraft:witch",
            "minecraft:firework",
            "minecraft:enchant",
            "minecraft:enchanted_hit",
        ]
        .map(|id| registrar.particle_key(id)),
    );
    for kind in [ANCIENT_ALTAR, ANCIENT_PEDESTAL] {
        let item = registrar.item_key(kind.material());
        registrar.item_part(item, Placement(kind));
    }
}

struct Placement(Kind);

#[mod_api::horizon_part]
impl ItemPart for Placement {
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        if !self.0.matches(host, event.stack) {
            return None;
        }
        let player = host.player(event.player)?;
        if research::can_use(host, player, &ALTAR_RESEARCH) {
            return None;
        }
        tell(
            host,
            player,
            "Unlock the Ancient Altar with /sf research ancient_altar.",
        );
        Some(InteractionResult::Fail)
    }

    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        if let Some(player) = event.placer
            && self.0.matches(host, event.stack)
            && research::can_use(host, player, &ALTAR_RESEARCH)
        {
            let marker = Compound::from([("id".into(), Tag::String(self.0.id().into()))]);
            let _ = DEVICE.set(host, Holder::BlockEntity(event.pos), marker);
        }
    }
}

pub(crate) fn restore_drops(level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>) {
    let Some(marker) = event.block_entity.and_then(|entity| {
        level.block_entity_saved_attachment(entity, DEVICE.namespace, DEVICE.name)
    }) else {
        return;
    };
    let Some(kind) = id(&marker) else { return };
    if let Some(key) = level.item_key_by_name(kind.material())
        && let Some(drop) = event.drops.iter_mut().find(|drop| drop.stack.item() == key)
    {
        kind.decorate(&mut drop.stack);
    }
    if let Some(Tag::Compound(item)) = marker.get(&"item".into())
        && let Some(stack) =
            crate::network_runtime::read_stack(item, |id| level.item_key_by_name(id))
        && stack.count() == 1
    {
        let pos = [event.pos.x, event.pos.y, event.pos.z].map(|axis| f64::from(axis) + 0.5);
        event
            .drops
            .push(ItemDrop::new(stack, pos, [0.0, 0.2, 0.0], 10));
    }
}

fn tell(host: &mut dyn ServerHost, player: PlayerRef, message: &str) {
    let _ = player.tell(host, Text::literal(message));
}

fn sound(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, id: &str, volume: f32, pitch: f32) {
    let _ = level.play_sound(
        host,
        id,
        pos.map(|v| f64::from(v) + 0.5),
        SoundSource::Blocks,
        volume,
        pitch,
    );
}

fn drop_item(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, stack: ItemStack) -> bool {
    let [x, y, z] = pos.map(|axis| f64::from(axis) + 0.5);
    level
        .spawn_item(host, x, y + 0.5, z, stack, 10)
        .is_ok_and(|item| item.is_some())
}

fn hand(player: PlayerRef) -> ItemPlace {
    ItemPlace::PlayerHand {
        player: player.uuid(),
        hand: Hand::Main,
    }
}

fn use_pedestal(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    pos: Pos,
    mut marker: Compound,
    use_item: bool,
) -> Result<(), &'static str> {
    let slot = "item".into();
    if marker.contains_key(&slot) {
        let item = ingredient(host, &marker).ok_or("The saved pedestal item is damaged.")?;
        let resource = ItemResource::of(&item);
        host.transaction(|host, tx| {
            let inserted = [true, false].into_iter().any(|occupied| {
                (0..36).any(|slot| {
                    let at = ItemPlace::PlayerSlot {
                        player: player.uuid(),
                        slot,
                    };
                    at.stack(host).is_some() == occupied && at.insert(host, tx, &resource, 1) == 1
                })
            });
            if !inserted && !drop_item(host, level, pos, item) {
                return Err("The pedestal could not return its item.");
            }
            marker.remove(&slot);
            DEVICE
                .set(host, Holder::BlockEntity(pos), marker)
                .map_err(|_| "The pedestal inventory is unavailable.")?;
            Ok(())
        })?;
        sound(host, level, pos, "minecraft:entity.item.pickup", 1.0, 1.0);
        return Ok(());
    }
    let held = hand(player);
    let Some(mut stack) = use_item.then(|| held.stack(host)).flatten() else {
        return Ok(());
    };
    let above = [
        pos[0],
        pos[1].checked_add(1).ok_or("Invalid pedestal height.")?,
        pos[2],
    ];
    let is_air = level.block_state(host, above).is_some_and(|state| {
        ["minecraft:air", "minecraft:cave_air", "minecraft:void_air"]
            .iter()
            .any(|id| host.block_key(id) == Some(state.block()))
    });
    if !is_air {
        return Err("The top of the Ancient Pedestal is obstructed.");
    }
    stack
        .set_count(1)
        .map_err(|_| "The pedestal item could not be read.")?;
    let saved = stack
        .to_nbt(host)
        .ok_or("The pedestal item could not be saved.")?;
    let creative = player.game_mode(host) == Some(GameMode::Creative);
    host.transaction(|host, tx| {
        if !creative && held.extract(host, tx, &ItemResource::of(&stack), 1) != 1 {
            return Err("The item in the player's hand changed.");
        }
        marker.insert(slot, Tag::Compound(saved));
        DEVICE
            .set(host, Holder::BlockEntity(pos), marker)
            .map_err(|_| "The pedestal inventory is unavailable.")?;
        watch_display(host, pos)
    })?;
    sound(host, level, pos, "minecraft:entity.item.pickup", 0.5, 0.5);
    Ok(())
}

struct Ritual {
    altar: Pos,
    due: i64,
    stage: i32,
    step_delay: i32,
    ring: [Compound; 8],
    output: Compound,
}

impl Ritual {
    fn from_nbt(data: &Compound) -> Option<Self> {
        let (
            Some(Tag::IntArray(altar)),
            Some(Tag::Long(due)),
            Some(Tag::Int(stage)),
            Some(Tag::List(ring)),
            Some(Tag::Compound(output)),
        ) = (
            data.get(&"altar".into()),
            data.get(&"due".into()),
            data.get(&"stage".into()),
            data.get(&"ring".into()),
            data.get(&"output".into()),
        )
        else {
            return None;
        };
        if !(0..=FINISH_STAGE).contains(stage) {
            return None;
        }
        let step_delay = match data.get(&"step_delay".into()) {
            None => DEFAULT_STEP_DELAY,
            Some(Tag::Int(delay)) if *delay >= 0 => *delay,
            _ => return None,
        };
        let ring: Vec<_> = ring
            .iter()
            .map(|tag| match tag {
                Tag::Compound(item) => Some(item.clone()),
                _ => None,
            })
            .collect::<Option<_>>()?;
        Some(Self {
            altar: altar.as_slice().try_into().ok()?,
            due: *due,
            stage: *stage,
            step_delay,
            ring: ring.try_into().ok()?,
            output: output.clone(),
        })
    }

    fn into_nbt(self) -> Compound {
        Compound::from([
            ("altar".into(), Tag::IntArray(self.altar.to_vec().into())),
            ("due".into(), Tag::Long(self.due)),
            ("stage".into(), Tag::Int(self.stage)),
            ("step_delay".into(), Tag::Int(self.step_delay)),
            (
                "ring".into(),
                Tag::List(self.ring.into_iter().map(Tag::Compound).collect()),
            ),
            ("output".into(), Tag::Compound(self.output)),
        ])
    }
}

fn rituals(host: &mut dyn ServerHost) -> Result<Compound, &'static str> {
    RITUALS
        .get_existing(host, Holder::Server)
        .map(|data| data.unwrap_or_default())
        .map_err(|_| "The Ancient Altar save is unavailable.")
}

fn dimension_rituals(records: &Compound, dimension: &str) -> Result<Compound, &'static str> {
    match records.get(&dimension.into()) {
        None => Ok(Compound::new()),
        Some(Tag::Compound(rituals)) => Ok(rituals.clone()),
        Some(_) => Err("The Ancient Altar save in this dimension is damaged."),
    }
}

fn locked(records: &Compound, pos: Pos) -> Result<bool, &'static str> {
    for value in records.values() {
        let Tag::Compound(record) = value else {
            return Err("The Ancient Altar ritual is damaged.");
        };
        let ritual = Ritual::from_nbt(record).ok_or("The Ancient Altar ritual is damaged.")?;
        let ring = pedestals(ritual.altar).ok_or("Invalid Ancient Altar position.")?;
        if ritual.altar == pos || ring.contains(&pos) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn start_ritual(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    altar: Pos,
    use_item: bool,
    mut all: Compound,
    mut active: Compound,
) -> Result<(), &'static str> {
    if !research::can_use(host, player, &ALTAR_RESEARCH) {
        return Err("Unlock the Ancient Altar with /sf research ancient_altar.");
    }
    let held = hand(player);
    let catalyst = use_item
        .then(|| held.stack(host))
        .flatten()
        .ok_or("Hold the Ancient Altar catalyst in your main hand.")?;
    let ring = pedestals(altar).ok_or("Invalid Ancient Altar position.")?;
    let mut items = Vec::with_capacity(8);
    for pos in ring {
        if locked(&active, pos)? {
            return Err("An Ancient Pedestal is in use.");
        }
        let (_, marker) = device(host, level, pos)
            .filter(|(kind, _)| *kind == ANCIENT_PEDESTAL)
            .ok_or("The Ancient Altar requires all eight pedestals.")?;
        items.push(
            ingredient(host, &marker)
                .ok_or("The Ancient Altar requires one item on each pedestal.")?,
        );
    }
    let items: [ItemStack; 8] = items.try_into().map_err(|_| "Incomplete pedestal ring.")?;
    let recipe =
        ritual_recipe(host, &catalyst, &items).ok_or("No Ancient Altar recipe matched.")?;
    if !research::can_use(host, player, recipe.research) {
        return Err(match recipe.research.id {
            "essence_of_afterlife" => "Unlock Necromancy with /sf research essence_of_afterlife.",
            _ => "Unlock Elemental Runes with /sf research ancient_runes.",
        });
    }
    let output = recipe
        .output
        .kind
        .stack(host, recipe.output.count)
        .and_then(|stack| stack.to_nbt(host))
        .ok_or("The ritual output could not be saved.")?;
    let saved: Vec<_> = items
        .iter()
        .map(|stack| stack.to_nbt(host))
        .collect::<Option<_>>()
        .ok_or("A pedestal item could not be saved.")?;
    let now = level
        .game_time(host)
        .ok_or("The Ancient Altar is not loaded.")?;
    let ritual = Ritual {
        altar,
        due: now.saturating_add(START_DELAY),
        stage: 0,
        step_delay: STEP_DELAY.load(Ordering::Relaxed),
        ring: saved.try_into().map_err(|_| "Incomplete pedestal ring.")?,
        output,
    };
    let [x, y, z] = altar;
    active.insert(
        format!("{x},{y},{z}").as_str().into(),
        Tag::Compound(ritual.into_nbt()),
    );
    let dimension = host.dimension().to_owned();
    all.insert(dimension.as_str().into(), Tag::Compound(active));
    let creative = player.game_mode(host) == Some(GameMode::Creative);
    host.transaction(|host, tx| {
        if !creative && held.extract(host, tx, &ItemResource::of(&catalyst), 1) != 1 {
            return Err("The catalyst in the player's hand changed.");
        }
        RITUALS
            .set(host, Holder::Server, all)
            .map_err(|_| "Could not save the Ancient Altar ritual.")
    })?;
    sound(
        host,
        level,
        altar,
        "minecraft:entity.illusioner.prepare_mirror",
        1.0,
        1.0,
    );
    Ok(())
}

pub(crate) fn use_at(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    pos: Pos,
    use_item: bool,
) -> bool {
    let Some((kind, marker)) = device(host, level, pos) else {
        return false;
    };
    let activate = |host: &mut dyn ServerHost| {
        let all = rituals(host)?;
        let active = dimension_rituals(&all, host.dimension())?;
        if locked(&active, pos)? {
            return Err("This Ancient Altar or Pedestal is in use.");
        }
        if kind == ANCIENT_ALTAR {
            start_ritual(host, player, level, pos, use_item, all, active)
        } else {
            use_pedestal(host, player, level, pos, marker, use_item)
        }
    };
    if let Err(message) = activate(host) {
        tell(host, player, message);
    }
    true
}

fn checked_pedestal(stage: i32) -> Option<usize> {
    (stage % 4 == 0 && (4..=32).contains(&stage)).then(|| (stage / 4 - 1) as usize)
}

fn display_uuid(dimension: &str, position: Pos) -> (EntityUuid, [i32; 4]) {
    let bytes = dimension
        .bytes()
        .chain(position.into_iter().flat_map(i32::to_be_bytes));
    let hash = |offset: u64| {
        bytes.clone().fold(offset, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
        })
    };
    let id =
        (u128::from(hash(0xcbf2_9ce4_8422_2325)) << 64) | u128::from(hash(0xa68e_91c7_3c25_1a9b));
    let words = [96, 64, 32, 0].map(|shift| (id >> shift) as i32);
    (EntityUuid(id), words)
}

fn watch_display(host: &mut dyn ServerHost, pos: Pos) -> Result<(), &'static str> {
    let dimension = host.dimension().to_owned();
    let mut records = DISPLAYS
        .get_existing(host, Holder::Server)
        .map_err(|_| "The pedestal display save is unavailable.")?
        .unwrap_or_default();
    let dimension_key = dimension.as_str().into();
    let entry = records
        .entry(dimension_key)
        .or_insert_with(|| Tag::Compound(Compound::new()));
    let Tag::Compound(positions) = entry else {
        return Err("The pedestal display save is damaged.");
    };
    let [x, y, z] = pos;
    positions.insert(
        format!("{x},{y},{z}").as_str().into(),
        Tag::IntArray(pos.to_vec().into()),
    );
    DISPLAYS
        .set(host, Holder::Server, records)
        .map_err(|_| "The pedestal display could not be saved.")
}

fn update_displays(host: &mut dyn ServerHost, level: LevelRef, dimension: &str) {
    let Ok(Some(mut records)) = DISPLAYS.get_existing(host, Holder::Server) else {
        return;
    };
    let dimension_key = dimension.into();
    let Some(Tag::Compound(positions)) = records.get_mut(&dimension_key) else {
        return;
    };
    let mut removed = Vec::new();
    for (key, value) in positions.iter() {
        let Tag::IntArray(position) = value else {
            continue;
        };
        let Ok(pos) = <Pos>::try_from(position.as_slice()) else {
            continue;
        };
        if !level.is_loaded(host, pos) {
            continue;
        }
        let (uuid, words) = display_uuid(dimension, pos);
        let live = host
            .entity_by_uuid(uuid)
            .and_then(|entity| host.display(entity).map(|display| (entity, display)));
        let desired = device(host, level, pos)
            .filter(|(kind, _)| *kind == ANCIENT_PEDESTAL)
            .and_then(|(_, marker)| ingredient(host, &marker));
        match (live, desired) {
            (None, Some(item)) => {
                let Some(saved) = item.to_nbt(host) else {
                    continue;
                };
                let [x, y, z] = pos.map(|value| f64::from(value) + 0.5);
                let nbt = Compound::from([
                    ("UUID".into(), Tag::IntArray(words.to_vec().into())),
                    ("item".into(), Tag::Compound(saved)),
                    ("item_display".into(), Tag::String("ground".into())),
                    ("billboard".into(), Tag::String("vertical".into())),
                    ("Invulnerable".into(), Tag::Byte(1)),
                ]);
                let _ = host.summon(dimension, "minecraft:item_display", [x, y + 0.7, z], nbt);
            }
            (Some((_, display)), Some(item)) => {
                if display.item(host).as_ref() != Some(&item) {
                    let _ = display.set_item(host, Some(item));
                }
            }
            (Some((entity, _)), None) => {
                if entity.discard(host) {
                    removed.push(key.clone());
                }
            }
            (None, None) => removed.push(key.clone()),
        }
    }
    if !removed.is_empty() {
        for key in removed {
            positions.remove(&key);
        }
        if positions.is_empty() {
            records.remove(&dimension_key);
        }
        let _ = DISPLAYS.set(host, Holder::Server, records);
    }
}

pub(crate) fn tick(host: &mut dyn ServerHost, level: LevelRef) {
    let Some(now) = level.game_time(host).filter(|_| level.in_place(host)) else {
        return;
    };
    let Some(dimension) = level.dimension().id(host) else {
        return;
    };
    if now.rem_euclid(5) == 0 {
        update_displays(host, level, &dimension);
    }
    let Ok(all) = rituals(host) else { return };
    let Ok(active) = dimension_rituals(&all, &dimension) else {
        return;
    };
    for (key, value) in active {
        let Tag::Compound(value) = value else {
            continue;
        };
        let Some(ritual) = Ritual::from_nbt(&value) else {
            continue;
        };
        if ritual.due > now || !level.is_loaded(host, ritual.altar) {
            continue;
        }
        let Ok(mut all) = rituals(host) else { return };
        let Ok(mut active) = dimension_rituals(&all, &dimension) else {
            return;
        };
        if active.get(&key) != Some(&Tag::Compound(value)) {
            continue;
        }
        let _ = host
            .transaction(|host, _| {
                let (next, effect) = advance(host, level, ritual, now)?;
                match next {
                    Some(next) => active.insert(key, Tag::Compound(next.into_nbt())),
                    None => active.remove(&key),
                };
                if active.is_empty() {
                    all.remove(&dimension.as_str().into());
                } else {
                    all.insert(dimension.as_str().into(), Tag::Compound(active));
                }
                RITUALS.set(host, Holder::Server, all).map_err(|_| ())?;
                Ok::<_, ()>(effect)
            })
            .map(|effect| effect.play(host, level));
    }
}

enum Effect {
    Stage(Pos, i32),
    Consume(Pos, Pos, i32),
    Abort(Pos),
    Finish(Pos),
}

impl Effect {
    fn play(self, host: &mut dyn ServerHost, level: LevelRef) {
        match self {
            Self::Stage(pos, stage) => ritual_particles(host, level, pos, stage),
            Self::Consume(altar, pedestal, stage) => {
                ritual_particles(host, level, altar, stage);
                sound(
                    host,
                    level,
                    pedestal,
                    "minecraft:entity.enderman.teleport",
                    1.0,
                    2.0,
                );
            }
            Self::Abort(pos) => sound(
                host,
                level,
                pos,
                "minecraft:entity.zombie.attack_iron_door",
                1.0,
                1.0,
            ),
            Self::Finish(pos) => {
                ritual_particles(host, level, pos, FINISH_STAGE);
                sound(
                    host,
                    level,
                    pos,
                    "minecraft:entity.zombie_villager.cure",
                    1.0,
                    1.0,
                );
            }
        }
    }
}

fn ritual_particles(host: &mut dyn ServerHost, level: LevelRef, altar: Pos, stage: i32) {
    let Some([witch, firework, enchant, hit]) = RITUAL_PARTICLES.get().copied() else {
        return;
    };
    let [x, y, z] = altar.map(|value| f64::from(value) + 0.5);
    level.send_particles(
        host,
        &ParticleOptions::simple(witch),
        None,
        [x, y + 0.8, z],
        [1.2, 0.0, 1.2],
        0.0,
        16,
    );
    level.send_particles(
        host,
        &ParticleOptions::simple(firework),
        None,
        [x, y + 0.8, z],
        [0.2, 0.0, 0.2],
        0.0,
        8,
    );
    let Some(ring) = pedestals(altar) else { return };
    for [x, y, z] in ring.into_iter().take((stage / 4).min(8) as usize) {
        let at = [f64::from(x) + 0.5, f64::from(y) + 1.5, f64::from(z) + 0.5];
        level.send_particles(
            host,
            &ParticleOptions::simple(enchant),
            None,
            at,
            [0.3, 0.2, 0.3],
            0.0,
            16,
        );
        level.send_particles(
            host,
            &ParticleOptions::simple(hit),
            None,
            at,
            [0.3, 0.2, 0.3],
            0.0,
            8,
        );
    }
}

fn advance(
    host: &mut dyn ServerHost,
    level: LevelRef,
    mut ritual: Ritual,
    now: i64,
) -> Result<(Option<Ritual>, Effect), ()> {
    let altar = ritual.altar;
    if ritual.stage == FINISH_STAGE {
        let stack = ItemStack::from_nbt(host, &ritual.output).ok_or(())?;
        return drop_item(host, level, altar, stack)
            .then_some((None, Effect::Finish(altar)))
            .ok_or(());
    }
    let effect = if let Some(index) = checked_pedestal(ritual.stage) {
        let pedestal = pedestals(altar).ok_or(())?[index];
        if !level.is_loaded(host, pedestal) {
            return Err(());
        }
        let Some((_, mut marker)) =
            device(host, level, pedestal).filter(|(kind, _)| *kind == ANCIENT_PEDESTAL)
        else {
            return Ok((None, Effect::Abort(altar)));
        };
        let current = marker.remove(&"item".into());
        if current != Some(Tag::Compound(ritual.ring[index].clone())) {
            return Ok((None, Effect::Abort(altar)));
        }
        DEVICE
            .set(host, Holder::BlockEntity(pedestal), marker)
            .map_err(|_| ())?;
        Effect::Consume(altar, pedestal, ritual.stage)
    } else {
        Effect::Stage(altar, ritual.stage)
    };
    ritual.stage += 1;
    ritual.due = now.saturating_add(i64::from(ritual.step_delay));
    Ok((Some(ritual), effect))
}

fn ritual_recipe(
    host: &dyn Host,
    catalyst: &ItemStack,
    ring: &[ItemStack; 8],
) -> Option<&'static CraftingRecipe> {
    CRAFTING
        .iter()
        .chain(crate::armor_forge::ALTAR_RECIPES)
        .find(|recipe| {
            recipe.inputs[4].is_some_and(|kind| kind.matches(host, catalyst))
                && (0..8).any(|rotation| {
                    PEDESTAL_PATTERN.iter().enumerate().all(|(offset, &cell)| {
                        recipe.inputs[cell]
                            .is_some_and(|kind| kind.matches(host, &ring[(offset + rotation) % 8]))
                    })
                })
        })
}

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    let lore = match id {
        "ANCIENT_PEDESTAL" => &["Part of the Ancient Altar"][..],
        "ANCIENT_ALTAR" => &["Multi-Block Altar for", "magical Crafting Processes"],
        _ => &[],
    };
    if !lore.is_empty() {
        let lines = [Tag::String("".into())]
            .into_iter()
            .chain(lore.iter().map(|line| text(line, "dark_purple", false)))
            .collect();
        components.insert("minecraft:lore".into(), Tag::List(lines));
    }
    let (element, element_color, rgb) = match id {
        "BLANK_RUNE" => ("", "dark_gray", 0),
        "ANCIENT_RUNE_AIR" => ("Air", "aqua", 0x00ffff),
        "ANCIENT_RUNE_EARTH" => ("Earth", "red", 0x702f07),
        "ANCIENT_RUNE_FIRE" => ("Fire", "dark_red", 0xff0000),
        "ANCIENT_RUNE_WATER" => ("Water", "dark_blue", 0x0000ff),
        "ANCIENT_RUNE_ENDER" => ("Ender", "dark_purple", 0x800080),
        "ANCIENT_RUNE_RAINBOW" => ("Rainbow", "light_purple", 0xff00ff),
        _ => return,
    };
    components.insert(
        "minecraft:firework_explosion".into(),
        Tag::compound([
            ("shape", Tag::String("burst".into())),
            ("colors", Tag::IntArray(vec![rgb].into())),
        ]),
    );
    components.insert(
        "minecraft:tooltip_display".into(),
        Tag::compound([(
            "hidden_components",
            Tag::List(vec![Tag::String("minecraft:firework_explosion".into())]),
        )]),
    );
    if !element.is_empty() {
        let bold = |value: &str, color: &str| {
            Tag::compound([
                ("text", Tag::String(value.into())),
                ("color", Tag::String(color.into())),
                ("bold", Tag::Byte(1)),
            ])
        };
        let name = Tag::compound([
            ("text", Tag::String("Ancient Rune ".into())),
            ("color", Tag::String("gray".into())),
            ("italic", Tag::Byte(0)),
            (
                "extra",
                Tag::List(vec![
                    bold("[", "dark_gray"),
                    bold(element, element_color),
                    bold("]", "dark_gray"),
                ]),
            ),
        ]);
        components.insert("minecraft:custom_name".into(), name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn paper_marker_archives_distinguish_absent_and_untyped_item_data() {
        let host = FakeHost::new();
        let empty: toml::Table = "[metadata]\nid = 'ANCIENT_PEDESTAL'".parse().unwrap();
        assert!(
            paper_ingredient(&host, ANCIENT_PEDESTAL, &empty)
                .unwrap()
                .is_none()
        );
        let untyped: toml::Table = "[metadata]\nid = 'ANCIENT_PEDESTAL'\nitem = 'opaque'"
            .parse()
            .unwrap();
        assert!(
            paper_ingredient(&host, ANCIENT_PEDESTAL, &untyped)
                .unwrap_err()
                .contains("remains archived")
        );
        let null: toml::Table =
            "null_metadata_keys = ['item']\n[metadata]\nid = 'ANCIENT_PEDESTAL'"
                .parse()
                .unwrap();
        assert!(
            paper_ingredient(&host, ANCIENT_PEDESTAL, &null)
                .unwrap()
                .is_none()
        );
        let wrong: toml::Table = "paper_inventory = ''\ninventory_preset = 'ANCIENT_ALTAR'"
            .parse()
            .unwrap();
        assert!(paper_ingredient(&host, ANCIENT_PEDESTAL, &wrong).is_err());
        let mut prefix = String::new();
        probe_name(
            &Tag::compound([
                ("text", Tag::String("ALTAR ".into())),
                (
                    "extra",
                    Tag::List(vec![Tag::String("Probe - 178899".into())]),
                ),
            ]),
            &mut prefix,
            0,
        );
        assert!(legacy_probe_text(&prefix));
        assert!(legacy_probe_text("§dALTAR §3Probe - §e79387"));
        assert!(!legacy_probe_text("My ALTAR Probe - 178899"));
    }

    #[test]
    fn the_paper_ring_accepts_rotations_but_not_mirrors_or_a_different_catalyst() {
        let mut host = FakeHost::new();
        let essence = CRAFTING
            .iter()
            .find(|recipe| recipe.output.kind == ESSENCE_OF_AFTERLIFE)
            .unwrap();
        host.registries
            .resolve_items(
                essence
                    .inputs
                    .into_iter()
                    .flatten()
                    .map(|kind| Identifier::parse(kind.material()).unwrap()),
            )
            .unwrap();
        let catalyst = NECROTIC_SKULL.stack(&host, 1).unwrap();
        let ring =
            PEDESTAL_PATTERN.map(|cell| essence.inputs[cell].unwrap().stack(&host, 1).unwrap());
        for turns in 0..8 {
            let rotated = std::array::from_fn(|index| ring[(index + turns) % 8].clone());
            assert_eq!(
                ritual_recipe(&host, &catalyst, &rotated)
                    .unwrap()
                    .output
                    .kind,
                ESSENCE_OF_AFTERLIFE
            );
        }
        let reflected = std::array::from_fn(|index| ring[7 - index].clone());
        assert!(ritual_recipe(&host, &catalyst, &reflected).is_none());
        let wrong = ENDER_LUMP_3.stack(&host, 1).unwrap();
        assert!(ritual_recipe(&host, &wrong, &ring).is_none());
        assert_eq!(pedestals([-32, 64, -8]).unwrap()[0], [-30, 64, -10]);
        assert_eq!(pedestals([-32, 64, -8]).unwrap()[7], [-32, 64, -11]);
        assert!(pedestals([i32::MAX, 64, 0]).is_none());
        assert_eq!(MAGIC_CRAFTING[0].output.count, 4);
        assert_eq!(essence.research.cost, 19);
    }

    #[test]
    fn a_saved_ritual_keeps_the_next_stage_and_locks_exactly_its_nine_positions() {
        let altar = [15, 70, -16];
        let output = Compound::from([("id".into(), Tag::String("minecraft:gunpowder".into()))]);
        let first = Ritual {
            altar,
            due: 600,
            stage: 20,
            step_delay: 3,
            ring: std::array::from_fn(|_| Compound::new()),
            output: output.clone(),
        };
        let saved = first.into_nbt();
        let recovered = Ritual::from_nbt(&saved).unwrap();
        assert_eq!(recovered.due, 600);
        assert_eq!(recovered.step_delay, 3);
        assert_eq!(recovered.output, output);
        assert_eq!(checked_pedestal(recovered.stage), Some(4));
        assert_eq!(
            (0..=FINISH_STAGE)
                .filter_map(checked_pedestal)
                .collect::<Vec<_>>(),
            (0..8).collect::<Vec<_>>()
        );
        let record = Compound::from([("15,70,-16".into(), Tag::Compound(saved))]);
        assert!(locked(&record, altar).unwrap());
        for pedestal in pedestals(altar).unwrap() {
            assert!(locked(&record, pedestal).unwrap());
        }
        assert!(!locked(&record, [15, 71, -16]).unwrap());
        assert_ne!(
            display_uuid("minecraft:overworld", altar).0,
            display_uuid("minecraft:the_nether", altar).0
        );
    }
}
