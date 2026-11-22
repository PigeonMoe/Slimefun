// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::BTreeMap,
    sync::{LazyLock, Mutex},
};

use mod_api::{
    AttachmentCodec, AttachmentType, ComponentId, Compound, ConfigFileError, ConfigPath, Hand,
    Holder, Host, HostTransaction, ITEM_ITEMS, Identifier, ItemPlace, ItemResource, ItemStack,
    MenuClosedEvent, MenuReplace, MenuSlotPlace, PermissionLevel, PlayerRef, Registrar,
    ResourceHandler, ServerHost, Tag, Text, Transaction,
};

use crate::{
    items::{
        ALUMINUM_INGOT, GOLD_6K, GOLD_10K, GOLD_14K, GOLD_16K, GOLD_22K, GOLD_24K, HeadTexture,
        Kind, item, item_id,
    },
    recipes::{CraftingRecipe, Research, Stack},
    research,
    storage::decode_archived_slots,
    thermal_machines::{COOLING, COOLING_UNIT},
};

mod cooler;
use cooler::COOLER;
pub(crate) use cooler::CoolerFeed;

#[path = "special_backpacks.rs"]
mod special;
pub(crate) use special::NECROMANCY;
pub(crate) use special::{ESSENCE_OF_AFTERLIFE, MAGIC_CRAFTING, NECROTIC_SKULL, keep_on_death};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct BackpackId {
    pub owner: u128,
    pub number: u32,
}

impl BackpackId {
    pub(crate) fn storage_key(self) -> String {
        self.storage_key_for(None)
    }

    fn storage_key_for(self, generation: Option<u128>) -> String {
        format!("{}{}", storage_prefix(self.owner, generation), self.number)
    }

    fn lore(self) -> String {
        let owner = format!("{:032x}", self.owner);
        format!(
            "ID: {}-{}-{}-{}-{}#{}",
            &owner[..8],
            &owner[8..12],
            &owner[12..16],
            &owner[16..20],
            &owner[20..],
            self.number
        )
    }
}

fn storage_prefix(owner: u128, generation: Option<u128>) -> String {
    match generation {
        Some(id) => format!("{owner:032x}/{id:032x}#"),
        None => format!("{owner:032x}#"),
    }
}

fn parse_owner(value: &str) -> Option<u128> {
    let mut parts = value.split('-');
    let mut uuid = String::with_capacity(32);
    for length in [8, 4, 4, 4, 12] {
        let part = parts.next()?;
        if part.len() != length || !part.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        uuid.push_str(part);
    }
    if parts.next().is_some() {
        return None;
    }
    u128::from_str_radix(&uuid, 16).ok()
}

fn number(value: &str) -> Option<u32> {
    if value.is_empty()
        || value.len() > 10
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok().filter(|n| *n <= i32::MAX as u32)
}

const BASIC_BACKPACKS: Research = Research {
    id: "backpacks",
    name: "Backpacks",
    cost: 15,
};
const WOVEN_RESEARCH: Research = Research {
    id: "woven_backpack",
    name: "Woven Backpack",
    cost: 19,
};
const GILDED_RESEARCH: Research = Research {
    id: "gilded_backpack",
    name: "Gilded Backpack",
    cost: 22,
};
const RADIANT_RESEARCH: Research = Research {
    id: "radiant_backpack",
    name: "Radiant Backpack",
    cost: 25,
};

pub(crate) const RESEARCHES: &[Research] = &[
    BASIC_BACKPACKS,
    WOVEN_RESEARCH,
    GILDED_RESEARCH,
    RADIANT_RESEARCH,
    special::SOULBOUND_STORAGE,
    special::ENDER_STORAGE,
    special::NECROMANCY,
];

const BACKPACK_TEXTURE: HeadTexture = HeadTexture {
    profile: [1142674294, 177618442, -2100126135, -1576084528],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzQwY2IxZTY3YjUxMmFiMmQ0YmYzZDdhY2UwZWFhZjYxYzMyY2Q0NjgxZGRjMzk4N2NlYjMyNjcwNmEzM2ZhIn19fQ==",
};

item!(
    SMALL_BACKPACK,
    "player_head",
    "Small Backpack",
    "yellow",
    None,
    Some(BACKPACK_TEXTURE)
);
item!(
    MEDIUM_BACKPACK,
    "player_head",
    "Backpack",
    "yellow",
    None,
    Some(BACKPACK_TEXTURE)
);
item!(
    LARGE_BACKPACK,
    "player_head",
    "Large Backpack",
    "yellow",
    None,
    Some(BACKPACK_TEXTURE)
);
item!(
    WOVEN_BACKPACK,
    "player_head",
    "Woven Backpack",
    "yellow",
    None,
    Some(BACKPACK_TEXTURE)
);
item!(
    GILDED_BACKPACK,
    "player_head",
    "Gilded Backpack",
    "yellow",
    None,
    Some(BACKPACK_TEXTURE)
);
item!(
    RADIANT_BACKPACK,
    "player_head",
    "Radiant Backpack",
    "yellow",
    None,
    Some(BACKPACK_TEXTURE)
);
item!(
    RESTORED_BACKPACK,
    "player_head",
    "Restored Backpack",
    "yellow",
    None,
    Some(HeadTexture {
        profile: [-1425698168, -825869582, -1134229925, 1957903403],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzljMzY4MWJmOGEyNzM4MjMyZmIzMDU1OTdmN2UyYTM0YTNhNWMxMzU2NzA1MjQ5ZTlhMzY1YjBiY2QwNDcwNWEifX19",
    })
);
item!(CLOTH, "paper", "Cloth", "aqua", None, None);

const LEATHER: Option<Kind> = Some(Kind::Vanilla("minecraft:leather"));

const fn casing(material: Option<Kind>, gold: Kind, center: Kind) -> [Option<Kind>; 9] {
    [
        material,
        None,
        material,
        Some(gold),
        Some(center),
        Some(gold),
        material,
        material,
        material,
    ]
}

const fn frame(gold: Kind, center: Kind) -> [Option<Kind>; 9] {
    [
        Some(gold),
        None,
        Some(gold),
        LEATHER,
        Some(center),
        LEATHER,
        Some(gold),
        None,
        Some(gold),
    ]
}

const fn recipe(
    inputs: [Option<Kind>; 9],
    kind: Kind,
    research: &'static Research,
) -> CraftingRecipe {
    CraftingRecipe {
        inputs,
        output: Stack { kind, count: 1 },
        research,
    }
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            Some(Kind::Vanilla("minecraft:white_wool")),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        output: Stack {
            kind: CLOTH,
            count: 8,
        },
        research: &crate::food_machines::FIRST_AID,
    },
    recipe(
        casing(LEATHER, GOLD_6K, Kind::Vanilla("minecraft:chest")),
        SMALL_BACKPACK,
        &BASIC_BACKPACKS,
    ),
    recipe(
        casing(LEATHER, GOLD_10K, SMALL_BACKPACK),
        MEDIUM_BACKPACK,
        &BASIC_BACKPACKS,
    ),
    recipe(
        casing(LEATHER, GOLD_14K, MEDIUM_BACKPACK),
        LARGE_BACKPACK,
        &BASIC_BACKPACKS,
    ),
    recipe(
        casing(Some(CLOTH), GOLD_16K, LARGE_BACKPACK),
        WOVEN_BACKPACK,
        &WOVEN_RESEARCH,
    ),
    recipe(
        frame(GOLD_22K, WOVEN_BACKPACK),
        GILDED_BACKPACK,
        &GILDED_RESEARCH,
    ),
    recipe(
        frame(GOLD_24K, GILDED_BACKPACK),
        RADIANT_BACKPACK,
        &RADIANT_RESEARCH,
    ),
    recipe(
        [
            Some(CLOTH),
            Some(CLOTH),
            Some(CLOTH),
            Some(ALUMINUM_INGOT),
            Some(COOLING_UNIT),
            Some(ALUMINUM_INGOT),
            Some(ALUMINUM_INGOT),
            Some(ALUMINUM_INGOT),
            Some(ALUMINUM_INGOT),
        ],
        COOLER,
        &COOLING,
    ),
];

fn tier(id: &str) -> Option<(u32, &'static Research)> {
    Some(match id {
        "SMALL_BACKPACK" => (9, &BASIC_BACKPACKS),
        "MEDIUM_BACKPACK" => (18, &BASIC_BACKPACKS),
        "LARGE_BACKPACK" => (27, &BASIC_BACKPACKS),
        "WOVEN_BACKPACK" => (36, &WOVEN_RESEARCH),
        "GILDED_BACKPACK" => (45, &GILDED_RESEARCH),
        "RADIANT_BACKPACK" => (54, &RADIANT_RESEARCH),
        "COOLER" => (cooler::SIZE, &COOLING),
        "BOUND_BACKPACK" => (36, &special::SOULBOUND_STORAGE),
        _ => return None,
    })
}

fn text(value: &str, color: &str, italic: bool) -> Tag {
    Tag::compound([
        ("text", Tag::String(value.into())),
        ("color", Tag::String(color.into())),
        ("italic", Tag::Byte(i8::from(italic))),
    ])
}

pub(crate) fn item_lore(id: &str) -> Option<Tag> {
    if id == special::ENDER_BACKPACK.id() {
        return Some(special::ender_lore());
    }
    if id == RESTORED_BACKPACK.id() {
        return Some(Tag::List(vec![
            Tag::String("".into()),
            text("Retrieve your lost items", "gray", false),
            text("ID: <ID>", "gray", false),
            Tag::String("".into()),
            open_lore(),
        ]));
    }
    let (size, _) = tier(id)?;
    let mut label = size.to_string();
    if size == 54 {
        label.push_str(" (Double chest)");
    }
    let mut lines = Vec::new();
    if id == COOLER.id() {
        for description in [
            "Allows you to store Juices/Smoothies",
            "and automatically consumes them when you are hungry",
            "and you have this in your Inventory",
        ] {
            lines.push(text(description, "white", false));
        }
    }
    lines.extend([
        Tag::String("".into()),
        Tag::compound([
            ("text", Tag::String("Size: ".into())),
            ("color", Tag::String("gray".into())),
            ("italic", Tag::Byte(0)),
            ("extra", Tag::List(vec![text(&label, "yellow", false)])),
        ]),
        text("ID: <ID>", "gray", false),
        Tag::String("".into()),
        open_lore(),
    ]);
    Some(Tag::List(lines))
}

fn open_lore() -> Tag {
    Tag::compound([
        ("text", Tag::String("Right Click".into())),
        ("color", Tag::String("yellow".into())),
        ("italic", Tag::Byte(0)),
        ("extra", Tag::List(vec![text(" to open", "gray", false)])),
    ])
}

fn allowed_contents(host: &dyn Host, stack: &ItemStack) -> bool {
    let material = stack.item().id(host);
    if material.is_empty()
        || matches!(material.as_str(), "minecraft:air" | "minecraft:shulker_box")
        || material
            .strip_prefix("minecraft:")
            .is_some_and(|path| path.ends_with("_shulker_box"))
    {
        return false;
    }
    item_id(stack).is_none_or(|id| {
        tier(&id).is_none() && !matches!(id.as_str(), "RESTORED_BACKPACK" | "ENDER_BACKPACK")
    })
}

fn append_literal(value: &Tag, output: &mut String, depth: u8) -> Option<()> {
    if depth >= 8 {
        return None;
    }
    match value {
        Tag::String(text) => {
            let text = text.to_string().ok()?;
            if output.len() + text.len() > 128 {
                return None;
            }
            output.push_str(&text);
        }
        Tag::Compound(node) => {
            append_literal(node.get(&"text".into())?, output, depth + 1)?;
            if let Some(extra) = node.get(&"extra".into()) {
                append_literal(extra, output, depth + 1)?;
            }
        }
        Tag::List(children) => {
            for child in children {
                append_literal(child, output, depth + 1)?;
            }
        }
        _ => return None,
    }
    Some(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Binding {
    Blank,
    Named(BackpackId),
}

fn binding(stack: &ItemStack) -> Result<Binding, &'static str> {
    let tags = stack.components().ok_or("The backpack has no ID lore.")?;
    let Some(Tag::List(lines)) = tags.as_nbt().get(&"minecraft:lore".into()) else {
        return Err("The backpack has no ID lore.");
    };
    let mut found = None;
    for line in lines {
        let mut literal = String::new();
        if append_literal(line, &mut literal, 0).is_none() {
            continue;
        }
        let literal = literal.strip_prefix("§7").unwrap_or(&literal);
        let Some(value) = literal.strip_prefix("ID: ") else {
            continue;
        };
        if found.is_some() {
            return Err("The backpack has more than one ID.");
        }
        let parsed = if value == "<ID>" {
            Binding::Blank
        } else {
            let (owner, id) = value.split_once('#').ok_or("Invalid Paper backpack ID.")?;
            Binding::Named(BackpackId {
                owner: parse_owner(owner).ok_or("Invalid Paper backpack owner UUID.")?,
                number: number(id).ok_or("Invalid Paper backpack number.")?,
            })
        };
        found = Some(parsed);
    }
    found.ok_or("The backpack has no ID lore.")
}

fn backpack_item(host: &dyn Host, stack: &ItemStack) -> Result<(Binding, u32), &'static str> {
    if stack.item().id(host) != "minecraft:player_head" {
        return Err("Hold a numbered Slimefun backpack.");
    }
    let capacity = item_id(stack)
        .as_deref()
        .and_then(tier)
        .map(|(size, _)| size)
        .ok_or("Hold a numbered Slimefun backpack.")?;
    if stack.count() != 1 {
        return Err("Backpacks must be held one at a time.");
    }
    Ok((binding(stack)?, capacity))
}

fn held_id(host: &dyn Host, stack: &ItemStack) -> Result<(BackpackId, u32), &'static str> {
    let (binding, capacity) = backpack_item(host, stack)?;
    match binding {
        Binding::Named(id) => Ok((id, capacity)),
        Binding::Blank => Err("This backpack has no existing Paper ID."),
    }
}

// A recovery head has no fixed tier. Its numbered record supplies the current capacity.
fn restored_id(host: &dyn Host, stack: &ItemStack) -> Result<BackpackId, &'static str> {
    if stack.count() != 1 || !RESTORED_BACKPACK.matches(host, stack) {
        return Err("Hold a single restored Slimefun backpack.");
    }
    match binding(stack)? {
        Binding::Named(id) => Ok(id),
        Binding::Blank => Err("A restored backpack must reference an existing backpack ID."),
    }
}

fn assign_item_id(stack: &mut ItemStack, id: BackpackId) -> Result<(), &'static str> {
    if binding(stack)? != Binding::Blank {
        return Err("The backpack already has a different ID.");
    }
    let mut components = stack
        .components()
        .ok_or("The backpack has no ID lore.")?
        .as_nbt()
        .clone();
    let Some(Tag::List(lines)) = components.get_mut(&"minecraft:lore".into()) else {
        return Err("The backpack has no ID lore.");
    };
    let line = lines
        .iter_mut()
        .find(|line| {
            let mut literal = String::new();
            append_literal(line, &mut literal, 0).is_some()
                && matches!(literal.as_str(), "ID: <ID>" | "§7ID: <ID>")
        })
        .ok_or("The backpack has no blank ID lore.")?;
    *line = text(&id.lore(), "gray", false);
    stack
        .set_components(components)
        .map_err(|_| "Could not update the backpack ID.")
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Backpack {
    pub size: u32,
    pub slots: BTreeMap<u32, ItemStack>,
    paper: bool,
}

fn valid_size(size: u32) -> bool {
    (9..=54).contains(&size) && size.is_multiple_of(9)
}

impl Backpack {
    pub(crate) fn to_nbt(&self, host: &dyn Host) -> Result<Compound, &'static str> {
        let mut items = Compound::new();
        if !valid_size(self.size) {
            return Err("Invalid backpack size.");
        }
        for (&slot, stack) in &self.slots {
            if slot >= self.size || !allowed_contents(host, stack) {
                return Err("The backpack contains an invalid slot or a prohibited item.");
            }
            let item = stack.to_nbt(host).ok_or("Cannot save the backpack item.")?;
            items.insert(slot.to_string().as_str().into(), Tag::Compound(item));
        }
        Ok(Compound::from([
            ("size".into(), Tag::Int(self.size as i32)),
            ("slots".into(), Tag::Compound(items)),
            ("paper".into(), Tag::Byte(i8::from(self.paper))),
        ]))
    }

    pub(crate) fn from_nbt(host: &dyn Host, data: &Compound) -> Result<Self, &'static str> {
        let Some(Tag::Int(size)) = data.get(&"size".into()) else {
            return Err("The saved backpack has no size.");
        };
        let size = u32::try_from(*size)
            .ok()
            .filter(|size| valid_size(*size))
            .ok_or("Invalid backpack size.")?;
        let Some(Tag::Compound(items)) = data.get(&"slots".into()) else {
            return Err("The saved backpack has no slot data.");
        };
        let Some(Tag::Byte(paper @ (0 | 1))) = data.get(&"paper".into()) else {
            return Err("The saved backpack has no recognized origin.");
        };
        let mut slots = BTreeMap::new();
        for (key, item) in items {
            let slot = key
                .to_string()
                .ok()
                .and_then(|key| number(&key))
                .filter(|&slot| slot < size)
                .ok_or("Invalid saved backpack slot.")?;
            let Tag::Compound(data) = item else {
                return Err("Invalid saved backpack item.");
            };
            let stack = ItemStack::from_nbt(host, data)
                .filter(|stack| allowed_contents(host, stack))
                .ok_or("The saved backpack contains an invalid or prohibited item.")?;
            slots.insert(slot, stack);
        }
        Ok(Self {
            size,
            slots,
            paper: *paper == 1,
        })
    }
}

pub(crate) fn stage_snapshot_bag<'a>(
    host: &dyn Host,
    number: u32,
    contents: impl ExactSizeIterator<Item = &'a [u8]>,
    mut decode: impl FnMut(&[u8]) -> Option<ItemStack>,
) -> Result<Compound, String> {
    let size = u32::try_from(contents.len())
        .ok()
        .filter(|&size| valid_size(size))
        .ok_or("同步档案中的背包容量无效。")?;
    let mut slots = BTreeMap::new();
    for (slot, bytes) in contents.enumerate() {
        if !bytes.is_empty() {
            let item = decode(bytes)
                .ok_or_else(|| format!("无法解码同步档案中背包 {number} 的第 {slot} 槽。"))?;
            slots.insert(slot as u32, item);
        }
    }
    // This record comes from an authoritative profile, not the legacy backpack recovery flow.
    Backpack {
        size,
        slots,
        paper: false,
    }
    .to_nbt(host)
    .map_err(|error| format!("无法保存同步档案中的背包 {number}。{error}"))
}

fn archived_records(
    profile: &toml::Table,
    owner: u128,
) -> Result<BTreeMap<u32, &toml::Table>, String> {
    if profile.get("format").and_then(toml::Value::as_integer) != Some(1) {
        return Err("Unsupported archived Paper backpack format.".into());
    }
    let owner = format!("{owner:032x}");
    if profile.get("player").and_then(toml::Value::as_str) != Some(owner.as_str()) {
        return Err("The archived Paper profile has a different owner UUID.".into());
    }
    if profile
        .get("paper_profile")
        .and_then(toml::Value::as_str)
        .is_none()
    {
        return Err("The original Paper profile is absent from this archive.".into());
    }
    if let Some(issues) = profile.get("paper_profile_issues") {
        let issues = issues
            .as_array()
            .ok_or("Invalid Paper profile diagnostics.")?;
        if let Some(first) = issues.first() {
            let first = first.as_str().ok_or("Invalid Paper profile diagnostic.")?;
            return Err(format!("The Paper profile is ambiguous. {first}"));
        }
    }
    let entries = profile
        .get("backpacks")
        .and_then(toml::Value::as_array)
        .ok_or("The Paper profile has no restorable backpack records.")?;
    let mut found = BTreeMap::new();
    for entry in entries {
        let entry = entry.as_table().ok_or("Invalid Paper backpack record.")?;
        let number = entry
            .get("id")
            .and_then(toml::Value::as_integer)
            .and_then(|n| i32::try_from(n).ok())
            .and_then(|n| u32::try_from(n).ok())
            .ok_or("Invalid archived Paper backpack number.")?;
        if found.insert(number, entry).is_some() {
            return Err(format!(
                "The Paper profile repeats backpack number {number}."
            ));
        }
    }
    Ok(found)
}

fn archived_backpack(
    profile: &toml::Table,
    id: BackpackId,
    expected_size: Option<u32>,
) -> Result<(&toml::Table, u32), String> {
    let records = archived_records(profile, id.owner)?;
    let entry = *records
        .get(&id.number)
        .ok_or("The Paper profile does not contain this backpack number.")?;
    let size = archived_size(entry).ok_or("The archived Paper backpack has an invalid size.")?;
    if expected_size.is_some_and(|expected| size != expected) {
        return Err("The Paper backpack size does not match the held backpack tier.".into());
    }
    Ok((entry, size))
}

fn archived_size(entry: &toml::Table) -> Option<u32> {
    entry
        .get("size")
        .and_then(toml::Value::as_integer)
        .and_then(|size| u32::try_from(size).ok())
        .filter(|&size| valid_size(size))
}

pub(crate) fn read_profile(
    host: &dyn ServerHost,
    owner: u128,
) -> Result<Option<toml::Table>, String> {
    let path = ConfigPath::new(&format!("storage-import/backpacks/{owner:032x}.toml"))
        .expect("hexadecimal UUID is a valid archive path");
    let bytes = match host.world_data_files().read(&path) {
        Ok(bytes) => bytes,
        Err(ConfigFileError::NotFound) => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Could not read the archived Paper backpack: {error}"
            ));
        }
    };
    let text =
        std::str::from_utf8(&bytes).map_err(|_| "The Paper backpack archive must be UTF-8.")?;
    text.parse()
        .map(Some)
        .map_err(|_| "The Paper backpack archive contains invalid TOML.".into())
}

fn load_paper(
    host: &dyn ServerHost,
    id: BackpackId,
    expected_size: Option<u32>,
) -> Result<Backpack, String> {
    if crate::pm_profile::committed_generation(host, id.owner)?.is_some() {
        return Err("本服当前的同步档案不包含这个背包。".into());
    }
    if let Some(profile) = read_profile(host, id.owner)? {
        let (row, size) = archived_backpack(&profile, id, expected_size)?;
        let slots = decode_archived_slots(host, row, size)?;
        return Ok(Backpack {
            size,
            slots,
            paper: true,
        });
    }
    crate::pm_profile::with_archive(host, id.owner, |profile| {
        let contents = profile
            .backpacks
            .get(&id.number)
            .ok_or("同步档案中没有此背包编号。")?;
        let size = contents.len() as u32;
        if expected_size.is_some_and(|expected| expected != size) {
            return Err("同步档案中的背包容量与当前背包不一致。".into());
        }
        let mut slots = BTreeMap::new();
        for (slot, encoded) in contents.iter().enumerate() {
            if !encoded.is_empty() {
                let item = host
                    .item_from_saved_gzip_nbt(encoded.to_vec())
                    .ok_or_else(|| format!("无法解码同步背包第 {slot} 槽的 Paper 物品。"))?;
                slots.insert(slot as u32, item);
            }
        }
        Ok(Backpack {
            size,
            slots,
            paper: true,
        })
    })?
    .ok_or_else(|| "There is no Paper archive for this backpack owner.".into())
}

/// The canonical record store is shared across dimensions and its writes join the item transaction.
pub(crate) trait BackpackStore: Send + Sync {
    fn load(&self, host: &mut dyn ServerHost, id: BackpackId) -> Result<Option<Compound>, String>;
    fn save(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        id: BackpackId,
        data: Compound,
    ) -> Result<(), String>;
}

static RECORDS: AttachmentType<Compound> = AttachmentType {
    namespace: "slimefun",
    name: "backpacks",
    default: Compound::new,
    codec: Some(AttachmentCodec {
        encode: |value| Ok(value.clone()),
        decode: |value| Ok(value.clone()),
    }),
    copy_on_death: false,
    sync: false,
};
static RECORDS_ID: LazyLock<Identifier> = LazyLock::new(|| {
    Identifier::parse("slimefun:backpacks").expect("the Slimefun backpack attachment ID")
});

pub(crate) struct GlobalBackpacks;

fn require_unfrozen_owner(host: &dyn ServerHost, owner: u128) -> Result<(), String> {
    if host.player_frozen_handover(owner).is_some() {
        return Err("背包所有者正在换服，暂时无法操作这个背包。".into());
    }
    Ok(())
}

impl BackpackStore for GlobalBackpacks {
    fn load(&self, host: &mut dyn ServerHost, id: BackpackId) -> Result<Option<Compound>, String> {
        let generation = crate::pm_profile::committed_generation(host, id.owner)?;
        let key = id.storage_key_for(generation);
        let lookup = |data: &Compound| data.get(&key.as_str().into()).cloned();
        let decoded = RECORDS
            .with_decoded(host, Holder::Server, |data| data.map(lookup))
            .map_err(|_| "The global backpack store is unavailable.")?;
        let record = match decoded {
            Some(record) => record,
            None => RECORDS
                .get_existing(host, Holder::Server)
                .map_err(|_| "The global backpack store is unavailable.")?
                .as_ref()
                .and_then(lookup),
        };
        match record {
            Some(Tag::Compound(record)) => Ok(Some(record)),
            Some(_) => Err("The global backpack record is corrupt.".into()),
            None => {
                let Some(generation) = generation else {
                    return Ok(None);
                };
                crate::pm_profile::with_committed_profile(host, id.owner, generation, |profile| {
                    profile
                        .backpacks
                        .get(&id.number)
                        .map(|slots| {
                            stage_snapshot_bag(
                                host,
                                id.number,
                                slots.iter().map(std::borrow::Cow::as_ref),
                                |bytes| host.item_from_saved_gzip_nbt(bytes.to_vec()),
                            )
                        })
                        .transpose()
                })
            }
        }
    }

    fn save(
        &self,
        host: &mut dyn ServerHost,
        _: &Transaction,
        id: BackpackId,
        record: Compound,
    ) -> Result<(), String> {
        // A recovered head may be held by another player while its owner transfers.
        require_unfrozen_owner(host, id.owner)?;
        let generation = crate::pm_profile::committed_generation(host, id.owner)?;
        let mut data = RECORDS
            .get_existing(host, Holder::Server)
            .map_err(|_| "The global backpack store is unavailable.")?
            .unwrap_or_default();
        data.insert(
            id.storage_key_for(generation).as_str().into(),
            Tag::Compound(record),
        );
        RECORDS
            .set(host, Holder::Server, data)
            .map_err(|_| "Could not write the global backpack record.".into())
    }
}

#[derive(Clone, Copy)]
pub(crate) enum CaptureBags<'a> {
    Legacy(Option<&'a toml::Table>),
    Committed(u128),
}

pub(crate) fn visit_snapshot_bags(
    host: &dyn ServerHost,
    owner: u128,
    bags: CaptureBags<'_>,
    limit: usize,
    mut visit: impl FnMut(&dyn ServerHost, u32, Backpack) -> Result<(), String>,
) -> Result<(), String> {
    let (generation, paper) = match bags {
        CaptureBags::Legacy(paper) => (None, paper),
        CaptureBags::Committed(generation) => (Some(generation), None),
    };
    if crate::pm_profile::committed_generation(host, owner)? != generation {
        return Err("采集期间背包档案版本已更换。".into());
    }
    let prefix = storage_prefix(owner, generation);
    let records = host
        .server_attachment_entries(&RECORDS_ID, &prefix, limit as u32)
        .map_err(|_| "无法读取全局背包存档。")?;
    let mut live = BTreeMap::new();
    for (key, record) in records {
        let number = key
            .to_string()
            .ok()
            .and_then(|key| key.strip_prefix(&prefix).and_then(number))
            .ok_or("存档中有无效的背包编号。")?;
        let Tag::Compound(record) = record else {
            return Err("无法读取存档中的背包。".into());
        };
        if live.insert(number, record).is_some() {
            return Err("存档中有重复的背包编号。".into());
        }
    }
    let archived = paper
        .map(|paper| archived_records(paper, owner))
        .transpose()?;
    let new_bags = archived.as_ref().map_or(0, |bags| {
        bags.keys().filter(|id| !live.contains_key(*id)).count()
    });
    if live.len() + new_bags > limit {
        return Err("Slimefun 同步档案的背包数量超出允许范围。".into());
    }
    for (id, row) in archived.into_iter().flatten() {
        if live.contains_key(&id) {
            continue;
        }
        let size = archived_size(row).ok_or("存档中的背包容量无效。")?;
        let slots = decode_archived_slots(host, row, size)?;
        visit(
            host,
            id,
            Backpack {
                size,
                slots,
                paper: true,
            },
        )?;
    }
    for (id, record) in live {
        visit(host, id, Backpack::from_nbt(host, &record)?)?;
    }
    Ok(())
}

/// Paper assigns the next ID after the last backpack in the owner's profile.
fn next_number(
    host: &mut dyn ServerHost,
    owner: u128,
    archived: Option<u32>,
) -> Result<u32, String> {
    let mut maximum = archived;
    let generation = crate::pm_profile::committed_generation(host, owner)?;
    let prefix = storage_prefix(owner, generation);
    if let Some(records) = RECORDS
        .get_existing(host, Holder::Server)
        .map_err(|_| "The global backpack store is unavailable.")?
    {
        for key in records.keys() {
            let Ok(key) = key.to_string() else { continue };
            if let Some(suffix) = key.strip_prefix(&prefix) {
                let current = number(suffix)
                    .ok_or("The owner already has an invalid live backpack number.")?;
                maximum = maximum.max(Some(current));
            }
        }
    }
    match maximum {
        None => Ok(0),
        Some(last) if last < i32::MAX as u32 => Ok(last + 1),
        Some(_) => Err("This owner's backpack numbers are exhausted.".into()),
    }
}

fn archived_max(host: &dyn ServerHost, owner: u128) -> Result<Option<u32>, String> {
    if let Some(generation) = crate::pm_profile::committed_generation(host, owner)? {
        return crate::pm_profile::with_committed_profile(host, owner, generation, |profile| {
            Ok(profile
                .backpacks
                .last_key_value()
                .map(|(&number, _)| number))
        });
    }
    let Some(profile) = read_profile(host, owner)? else {
        return Ok(crate::pm_profile::with_archive(host, owner, |profile| {
            Ok(profile
                .backpacks
                .last_key_value()
                .map(|(&number, _)| number))
        })?
        .flatten());
    };
    Ok(archived_records(&profile, owner)?
        .last_key_value()
        .map(|(&number, _)| number))
}

fn create_record(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    owner: u128,
    size: u32,
    reserved: Option<u32>,
) -> Result<BackpackId, String> {
    let id = BackpackId {
        owner,
        number: next_number(host, owner, reserved)?,
    };
    if GlobalBackpacks.load(host, id)?.is_some() {
        return Err("A backpack already has the new identity.".into());
    }
    let record = Backpack {
        size,
        slots: BTreeMap::new(),
        paper: false,
    }
    .to_nbt(host)?;
    GlobalBackpacks.save(host, tx, id, record)?;
    Ok(id)
}

fn initialize_held(
    host: &mut dyn ServerHost,
    place: &ItemPlace,
    owner: u128,
    reserved: Option<u32>,
) -> Result<(BackpackId, u32), String> {
    host.transaction(|host, tx| {
        let mut stack = place
            .stack(host)
            .ok_or("The backpack left the player's hand.")?;
        let (binding, size) = backpack_item(host, &stack)?;
        if binding != Binding::Blank {
            return Err("The held backpack already has an ID.".into());
        }
        let id = create_record(host, tx, owner, size, reserved)?;
        assign_item_id(&mut stack, id)?;
        let changed = place.exchange(host, tx, &ItemResource::of(&stack), 1);
        if changed != 1 {
            return Err("Could not bind the backpack in the player's hand.".into());
        }
        let current = place
            .stack(host)
            .ok_or("The bound backpack is missing from the hand.")?;
        if held_id(host, &current).ok() != Some((id, size)) {
            return Err("The hand did not keep the bound backpack.".into());
        }
        Ok((id, size))
    })
}

fn upgrade_bound(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    id: BackpackId,
    old_size: u32,
    size: u32,
) -> Result<(), String> {
    if !valid_size(size) || size < old_size {
        return Err("A backpack upgrade cannot shrink its capacity.".into());
    }
    let mut record =
        live_bag(host, id, old_size)?.ok_or("Restore this Paper backpack before upgrading it.")?;
    if size == old_size {
        return Ok(());
    }
    record.size = size;
    let saved = record.to_nbt(host)?;
    GlobalBackpacks.save(host, tx, id, saved)
}

pub(crate) fn prepare_craft(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    player: PlayerRef,
    slots: &[Option<ItemStack>; 9],
    output: &mut ItemStack,
) -> Result<(), String> {
    let Some((size, _)) = item_id(output).as_deref().and_then(tier) else {
        return Ok(());
    };
    let input = slots
        .iter()
        .flatten()
        .find(|item| item_id(item).as_deref().and_then(tier).is_some());
    let source = input.map(binding).transpose()?;
    let id = match source {
        Some(Binding::Named(_)) => {
            let previous = input.expect("a named binding was read from a backpack");
            let (id, old_size) = held_id(host, previous)?;
            upgrade_bound(host, tx, id, old_size, size)?;
            id
        }
        Some(Binding::Blank) | None => {
            let reserved = archived_max(host, player.uuid())?;
            create_record(host, tx, player.uuid(), size, reserved)?
        }
    };
    assign_item_id(output, id)?;
    if let Some(source) = input {
        carry_soulbound(source, output)?;
    }
    Ok(())
}

pub(crate) fn prepare_automated_craft<'a>(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    mut inputs: impl Iterator<Item = &'a ItemStack>,
    output: &mut ItemStack,
) -> Result<(), String> {
    let Some((size, _)) = item_id(output).as_deref().and_then(tier) else {
        return Ok(());
    };
    let Some(source) = inputs.find(|stack| item_id(stack).as_deref().and_then(tier).is_some())
    else {
        return Ok(());
    };
    if let (Binding::Named(id), old_size) = backpack_item(host, source)? {
        upgrade_bound(host, tx, id, old_size, size)?;
        assign_item_id(output, id)?;
    }
    carry_soulbound(source, output)?;
    Ok(())
}

const SOULBOUND_KEY: &str = "slimefun:soulbound";

fn soulbound_marker(source: &ItemStack) -> Option<Tag> {
    let components = source.components()?;
    let flag = components
        .as_nbt()
        .get(&"minecraft:custom_data".into())
        .and_then(|data| data.get("PublicBukkitValues"))
        .and_then(|data| data.get(SOULBOUND_KEY))
        .filter(|tag| matches!(tag, Tag::Byte(_)))
        .cloned();
    if flag.is_some() {
        return flag;
    }
    let has_lore = components
        .as_nbt()
        .get(&"minecraft:lore".into())
        .is_some_and(|lore| {
            let Tag::List(lines) = lore else { return false };
            lines.iter().any(|line| {
                let mut literal = String::new();
                append_literal(line, &mut literal, 0).is_some()
                    && matches!(literal.as_str(), "Soulbound" | "§7Soulbound")
            })
        });
    has_lore.then_some(Tag::Byte(1))
}

fn carry_soulbound(source: &ItemStack, output: &mut ItemStack) -> Result<(), &'static str> {
    let Some(flag) = soulbound_marker(source) else {
        return Ok(());
    };
    let mut components = output
        .components()
        .ok_or("The upgraded backpack has no components.")?
        .as_nbt()
        .clone();
    let Some(Tag::List(lines)) = components.get_mut(&"minecraft:lore".into()) else {
        return Err("The upgraded backpack has no lore.");
    };
    lines.push(text("Soulbound", "gray", false));
    let Some(Tag::Compound(data)) = components.get_mut(&"minecraft:custom_data".into()) else {
        return Err("The upgraded backpack has no Slimefun item data.");
    };
    let Some(Tag::Compound(values)) = data.get_mut(&"PublicBukkitValues".into()) else {
        return Err("The upgraded backpack has no Slimefun item data.");
    };
    values.insert(SOULBOUND_KEY.into(), flag);
    output
        .set_components(components)
        .map_err(|_| "Could not preserve the Soulbound marker.")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Restored {
    Installed,
    AlreadyInstalled,
}

fn bind_paper(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    store: &dyn BackpackStore,
    id: BackpackId,
    expected_size: Option<u32>,
    load: impl FnOnce(&dyn ServerHost, BackpackId, Option<u32>) -> Result<Backpack, String>,
) -> Result<Restored, String> {
    if let Some(data) = store.load(host, id)? {
        let current = Backpack::from_nbt(host, &data)?;
        return if current.paper && expected_size.is_none_or(|size| current.size == size) {
            Ok(Restored::AlreadyInstalled)
        } else {
            Err("A different live backpack already has this identity.".into())
        };
    }
    let imported = load(host, id, expected_size)?;
    if !imported.paper || expected_size.is_some_and(|size| imported.size != size) {
        return Err("The archived backpack does not match this item.".into());
    }
    let data = imported.to_nbt(host)?;
    store.save(host, tx, id, data)?;
    Ok(Restored::Installed)
}

fn may_restore(id: BackpackId, holder: u128, permissions: PermissionLevel) -> bool {
    id.owner == holder || permissions.level() >= PermissionLevel::Gamemasters.level()
}

pub(crate) fn restore_held(
    host: &mut dyn ServerHost,
    store: &dyn BackpackStore,
    player: PlayerRef,
    hand: Hand,
    permission: PermissionLevel,
) -> Result<Restored, String> {
    host.transaction(|host, tx| {
        let hand = ItemPlace::PlayerHand {
            player: player.uuid(),
            hand,
        };
        let stack = hand
            .stack(host)
            .ok_or("Hold the Paper backpack to restore it.")?;
        let (id, size) = if RESTORED_BACKPACK.matches(host, &stack) {
            (restored_id(host, &stack)?, None)
        } else {
            let (id, size) = held_id(host, &stack)?;
            (id, Some(size))
        };
        if !may_restore(id, player.uuid(), permission) {
            return Err(
                "The original owner or a level-2 operator must restore this backpack.".into(),
            );
        }
        require_unfrozen_owner(host, id.owner)?;
        bind_paper(host, tx, store, id, size, load_paper)
    })
}

pub(crate) fn recover_lost(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    permission: PermissionLevel,
    owner: &str,
    number: &str,
) -> Result<(), String> {
    if permission.level() < PermissionLevel::Gamemasters.level() {
        return Err("Recovering a lost backpack requires level-2 operator permission.".into());
    }
    let number = (!number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| number.parse::<i32>().ok())
        .flatten()
        .ok_or("Invalid backpack number.")? as u32;
    let owner = parse_owner(owner)
        .or_else(|| host.players().known_player_uuid(owner))
        .ok_or("Unknown backpack owner. Use the original player's name or UUID.")?;
    issue_recovered(host, player, BackpackId { owner, number }, load_paper)
}

fn issue_recovered(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    id: BackpackId,
    load: impl FnOnce(&dyn ServerHost, BackpackId, Option<u32>) -> Result<Backpack, String>,
) -> Result<(), String> {
    require_unfrozen_owner(host, id.owner)?;
    let mut head = RESTORED_BACKPACK
        .stack(host, 1)
        .ok_or("Could not create the restored backpack head.")?;
    assign_item_id(&mut head, id)?;
    let resource = ItemResource::of(&head);
    let maximum = max_slot(host, &resource);
    if maximum == 0 {
        return Err("Could not determine the restored backpack stack size.".into());
    }
    host.transaction(|host, tx| {
        if load_bag(host, id)?.is_none() {
            bind_paper(host, tx, &GlobalBackpacks, id, None, load)?;
        }
        let mut first_empty = None;
        for slot in 0..36 {
            let place = ItemPlace::PlayerSlot {
                player: player.uuid(),
                slot,
            };
            match place.stack(host) {
                Some(stack) if stack.count() < maximum && resource.matches(&stack) => {
                    if place.insert(host, tx, &resource, 1) == 1 {
                        return Ok(());
                    }
                }
                None if first_empty.is_none() => first_empty = Some(place),
                _ => {}
            }
        }
        first_empty
            .is_some_and(|place| place.insert(host, tx, &resource, 1) == 1)
            .then_some(())
            .ok_or_else(|| "Make room in your inventory for the restored backpack.".into())
    })
}

fn load_bag(host: &mut dyn ServerHost, id: BackpackId) -> Result<Option<Backpack>, String> {
    GlobalBackpacks.load(host, id)?.map_or(Ok(None), |data| {
        Backpack::from_nbt(host, &data)
            .map(Some)
            .map_err(str::to_owned)
    })
}

fn live_bag(
    host: &mut dyn ServerHost,
    id: BackpackId,
    size: u32,
) -> Result<Option<Backpack>, String> {
    let Some(bag) = load_bag(host, id)? else {
        return Ok(None);
    };
    if bag.size != size {
        return Err("The live backpack size does not match the held backpack tier.".into());
    }
    Ok(Some(bag))
}

fn held_record(host: &mut dyn ServerHost, stack: &ItemStack) -> Option<(BackpackId, Backpack)> {
    if RESTORED_BACKPACK.matches(host, stack) {
        let id = restored_id(host, stack).ok()?;
        return Some((id, load_bag(host, id).ok()??));
    }
    let (id, size) = held_id(host, stack).ok()?;
    Some((id, live_bag(host, id, size).ok()??))
}

pub(crate) fn register(registrar: &mut dyn Registrar) {
    special::register(registrar);
    registrar.item_capability(&ITEM_ITEMS, "minecraft:player_head", |host, at| {
        if !matches!(at, ItemPlace::PlayerHand { .. }) {
            return None;
        }
        let stack = at.stack(host)?;
        let (id, bag) = held_record(host, &stack)?;
        let generation = crate::pm_profile::committed_generation(host, id.owner).ok()?;
        Some(Box::new(BackpackInventory {
            at: at.clone(),
            id,
            generation,
            size: bag.size,
            cooler: COOLER.matches(host, &stack),
        }))
    });
}

struct BackpackInventory {
    at: ItemPlace,
    id: BackpackId,
    generation: Option<u128>,
    size: u32,
    cooler: bool,
}

impl BackpackInventory {
    fn accepts(&self, host: &dyn Host, stack: &ItemStack) -> bool {
        if self.cooler {
            crate::juice::identify(host, stack).is_some()
        } else {
            allowed_contents(host, stack)
        }
    }

    fn read(&self, host: &mut dyn ServerHost) -> Option<Backpack> {
        if !self.current_generation(host) {
            return None;
        }
        let held = self.at.stack(host)?;
        let (id, bag) = held_record(host, &held)?;
        (id == self.id && bag.size == self.size && self.cooler == COOLER.matches(host, &held))
            .then_some(bag)
    }

    fn write(&self, host: &mut dyn ServerHost, tx: &Transaction, bag: &Backpack) -> bool {
        if !self.current_generation(host) {
            return false;
        }
        let Ok(data) = bag.to_nbt(host) else {
            return false;
        };
        GlobalBackpacks.save(host, tx, self.id, data).is_ok()
    }

    fn current_generation(&self, host: &dyn ServerHost) -> bool {
        if !crate::pm_profile::committed_generation(host, self.id.owner)
            .is_ok_and(|active| active == self.generation)
        {
            return false;
        }
        let ItemPlace::PlayerHand { player, .. } = self.at else {
            return false;
        };
        let open = VIEWERS.lock().unwrap().get(&player).copied();
        !open.is_some_and(|open| {
            open.id == self.id
                && open.generation != self.generation
                && host.menu(player, open.window).is_some()
        })
    }
}

fn max_slot(host: &dyn Host, resource: &ItemResource) -> u32 {
    let key = ComponentId::new("minecraft:max_stack_size").expect("built-in item component");
    match resource.value(host, &key) {
        Some(Tag::Int(max @ 1..=99)) => (max as u32).min(64),
        _ => host.max_stack_size(resource.item()).unwrap_or(0).min(64),
    }
}

impl ResourceHandler<ItemResource> for BackpackInventory {
    fn size(&self, host: &mut dyn ServerHost) -> usize {
        self.read(host).map_or(0, |bag| bag.size as usize)
    }

    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemResource> {
        let slot = u32::try_from(index).ok()?;
        Some(ItemResource::of(self.read(host)?.slots.get(&slot)?))
    }

    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        let Ok(slot) = u32::try_from(index) else {
            return 0;
        };
        self.read(host)
            .and_then(|bag| bag.slots.get(&slot).map(ItemStack::count))
            .unwrap_or(0)
    }

    fn capacity(&self, host: &mut dyn ServerHost, index: usize, resource: &ItemResource) -> u32 {
        if index >= self.size as usize
            || !self.accepts(host, resource.as_stack())
            || self.read(host).is_none()
        {
            return 0;
        }
        max_slot(host, resource)
    }

    fn insert(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        index: usize,
        resource: &ItemResource,
        amount: u32,
    ) -> u32 {
        let Ok(slot) = u32::try_from(index) else {
            return 0;
        };
        let Some(mut bag) = self.read(host) else {
            return 0;
        };
        if slot >= bag.size || !self.accepts(host, resource.as_stack()) {
            return 0;
        }
        let resident = bag.slots.get(&slot);
        if resident.is_some_and(|stack| !resource.matches(stack)) {
            return 0;
        }
        let count = resident.map_or(0, ItemStack::count);
        let moved = max_slot(host, resource).saturating_sub(count).min(amount);
        if moved == 0 {
            return 0;
        }
        let Some(stack) = resource.to_stack(count + moved) else {
            return 0;
        };
        bag.slots.insert(slot, stack);
        if self.write(host, tx, &bag) { moved } else { 0 }
    }

    fn extract(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        index: usize,
        resource: &ItemResource,
        amount: u32,
    ) -> u32 {
        let Ok(slot) = u32::try_from(index) else {
            return 0;
        };
        let Some(mut bag) = self.read(host) else {
            return 0;
        };
        let Some(stack) = bag
            .slots
            .get_mut(&slot)
            .filter(|stack| resource.matches(stack))
        else {
            return 0;
        };
        let moved = stack.count().min(amount);
        if moved == 0 {
            return 0;
        }
        if moved == stack.count() {
            bag.slots.remove(&slot);
        } else if stack.set_count(stack.count() - moved).is_err() {
            return 0;
        }
        if self.write(host, tx, &bag) { moved } else { 0 }
    }
}

#[derive(Clone, Copy)]
struct OpenBackpack {
    id: BackpackId,
    window: i32,
    generation: Option<u128>,
}

static VIEWERS: Mutex<BTreeMap<u128, OpenBackpack>> = Mutex::new(BTreeMap::new());

#[derive(Default)]
pub(crate) struct BackpackMenus;

impl BackpackMenus {
    pub(crate) fn clear(&self) {
        VIEWERS.lock().unwrap().clear();
        crate::pm_profile::clear_committed_cache();
    }

    pub(crate) fn closed(&self, event: &MenuClosedEvent) {
        let mut viewers = VIEWERS.lock().unwrap();
        if viewers
            .get(&event.uuid)
            .is_some_and(|open| event.window == open.window)
        {
            viewers.remove(&event.uuid);
        }
    }

    fn refresh_cooler(&self, host: &mut dyn ServerHost, id: BackpackId) {
        let viewer = VIEWERS
            .lock()
            .unwrap()
            .iter()
            .find_map(|(&viewer, &open)| (open.id == id).then_some((viewer, open)));
        if let Some((viewer, open)) = viewer
            && host.menu(viewer, open.window).is_some()
            && crate::pm_profile::committed_generation(host, id.owner)
                .is_ok_and(|active| active == open.generation)
            && let Ok(Some(mut bag)) = live_bag(host, id, cooler::SIZE)
        {
            let slots = (0..bag.size).map(|slot| bag.slots.remove(&slot)).collect();
            let _ = host.menu_set_items(viewer, open.window, slots);
        }
    }

    pub(crate) fn use_held(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        hand: Hand,
    ) -> bool {
        let place = ItemPlace::PlayerHand {
            player: player.uuid(),
            hand,
        };
        let Some(stack) = place.stack(host) else {
            return false;
        };
        if special::open_ender(host, player, &stack) {
            return true;
        }
        let Some(item) = item_id(&stack) else {
            return false;
        };
        let missing = "Restore this Paper backpack with /sf restore-backpack while holding it in your main hand.";
        let target = if item == RESTORED_BACKPACK.id() {
            restored_id(host, &stack)
                .map_err(str::to_owned)
                .and_then(|id| {
                    require_unfrozen_owner(host, id.owner)?;
                    let bag = load_bag(host, id)?.ok_or(missing)?;
                    Ok((id, bag.size))
                })
        } else if let Some((_, required)) = tier(&item) {
            backpack_item(host, &stack)
                .map_err(str::to_owned)
                .and_then(|(binding, size)| {
                    if !research::can_use(host, player, required) {
                        return Err(format!(
                            "Unlock this research with /sf research {}.",
                            required.id
                        ));
                    }
                    let owner = match binding {
                        Binding::Named(id) => id.owner,
                        Binding::Blank => player.uuid(),
                    };
                    require_unfrozen_owner(host, owner)?;
                    let id = match binding {
                        Binding::Named(id) => id,
                        Binding::Blank => {
                            let reserved = archived_max(host, player.uuid())?;
                            initialize_held(host, &place, player.uuid(), reserved)?.0
                        }
                    };
                    live_bag(host, id, size)?.ok_or(missing)?;
                    Ok((id, size))
                })
        } else {
            return false;
        };
        let result = target.and_then(|(id, size)| {
            let generation = crate::pm_profile::committed_generation(host, id.owner)?;
            {
                let mut viewers = VIEWERS.lock().unwrap();
                viewers.retain(|uuid, open| host.menu(*uuid, open.window).is_some());
                if viewers.iter().any(|(&uuid, open)| {
                    open.id == id && open.generation == generation && uuid != player.uuid()
                }) {
                    return Err("Someone else is already using this backpack.".into());
                }
            }
            let window = host
                .open_menu(
                    player.uuid(),
                    (size / 9) as u8,
                    Text::literal(format!("Backpack [{size} Slots]")),
                    Vec::new(),
                    MenuReplace::CloseFirst,
                )
                .map_err(|_| "Could not open the backpack menu.")?;
            let Some(window) = window else { return Ok(()) };
            let slots = (0..size as usize)
                .map(|slot| MenuSlotPlace::HeldItem { hand, slot })
                .collect();
            if player.back_mod_menu_slots(host, window, slots).is_err() {
                host.close_menu(player.uuid());
                return Err("The held backpack could not be bound to its menu.".into());
            }
            VIEWERS.lock().unwrap().insert(
                player.uuid(),
                OpenBackpack {
                    id,
                    window,
                    generation,
                },
            );
            Ok(())
        });
        if let Err(message) = result {
            let _ = player.tell(host, Text::literal(message));
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    const OWNER_UUID: &str = "341da016-f646-4030-af46-6e652265fa34";
    const OWNER: u128 = 0x341da016f6464030af466e652265fa34;
    const ID: BackpackId = BackpackId {
        owner: OWNER,
        number: 3,
    };
    const RECIPIENT: u128 = 1;

    fn string(text: &str) -> Tag {
        Tag::String(text.into())
    }

    fn material(host: &dyn Host, name: &str, count: u32) -> ItemStack {
        let id = Identifier::parse(name).unwrap();
        ItemStack::new(host.item_key(&id).unwrap(), count).unwrap()
    }

    fn fixture() -> FakeHost {
        let mut host = FakeHost::new();
        let ids = [
            "minecraft:player_head",
            "minecraft:diamond",
            "minecraft:red_shulker_box",
        ]
        .map(|id| Identifier::parse(id).unwrap());
        host.registries.resolve_items(ids).unwrap();
        host
    }

    fn bag(host: &dyn Host, id: &str, lines: Vec<Tag>) -> ItemStack {
        let mut stack = material(host, "minecraft:player_head", 1);
        stack
            .set_components(Compound::from([
                (
                    "minecraft:custom_data".into(),
                    Tag::compound([(
                        "PublicBukkitValues",
                        Tag::compound([(crate::items::ID_KEY, string(id))]),
                    )]),
                ),
                ("minecraft:lore".into(), Tag::List(lines)),
            ]))
            .unwrap();
        stack
    }

    fn tagged_lines() -> Vec<Tag> {
        vec![
            string("Size: 9"),
            Tag::compound([
                ("text", string("")),
                (
                    "extra",
                    Tag::List(vec![Tag::compound([
                        ("text", string(&format!("ID: {OWNER_UUID}#3"))),
                        ("color", string("gray")),
                    ])]),
                ),
            ]),
        ]
    }

    #[test]
    fn borrowed_backpack_transfers_are_rolled_back_while_its_owner_is_frozen() {
        let mut host = fixture();
        host.join(RECIPIENT, 7);
        let borrower = (&host as &dyn Host).player(RECIPIENT).unwrap();
        let diamond = material(&host, "minecraft:diamond", 2);
        let resource = ItemResource::of(&diamond);
        let record = Backpack {
            size: 9,
            slots: BTreeMap::from([(8, diamond.clone())]),
            paper: false,
        };
        let saved = record.to_nbt(&host).unwrap();
        host.transaction(|host, tx| GlobalBackpacks.save(host, tx, ID, saved.clone()))
            .unwrap();
        recover_lost(
            &mut host,
            borrower,
            PermissionLevel::Gamemasters,
            OWNER_UUID,
            "3",
        )
        .unwrap();
        let bag = BackpackInventory {
            at: ItemPlace::PlayerHand {
                player: RECIPIENT,
                hand: Hand::Main,
            },
            id: ID,
            generation: None,
            size: 9,
            cooler: false,
        };
        let source = ItemPlace::PlayerSlot {
            player: RECIPIENT,
            slot: 1,
        };
        let destination = ItemPlace::PlayerSlot {
            player: RECIPIENT,
            slot: 2,
        };
        host.players
            .get_mut(RECIPIENT)
            .unwrap()
            .slots
            .insert(1, diamond.clone());

        // The borrower already has a valid backing inventory when the owner freezes.
        assert_eq!(bag.amount(&mut host, 8), 2);
        host.join(OWNER, 6);
        host.frozen_player_handovers.insert(OWNER, 41);
        let refusal = require_unfrozen_owner(&host, OWNER).unwrap_err();
        assert_eq!(
            host.transaction(|host, tx| GlobalBackpacks.save(host, tx, ID, Compound::new())),
            Err(refusal.clone())
        );
        assert_eq!(
            recover_lost(
                &mut host,
                borrower,
                PermissionLevel::Gamemasters,
                OWNER_UUID,
                "3"
            ),
            Err(refusal.clone())
        );
        assert_eq!(
            restore_held(
                &mut host,
                &GlobalBackpacks,
                borrower,
                Hand::Main,
                PermissionLevel::Gamemasters
            ),
            Err(refusal)
        );

        let withdraw = |host: &mut FakeHost| {
            host.transaction(|host, tx| {
                // Quick-move inserts at the destination first and aborts if extraction fails.
                let moved = destination.insert(host, tx, &resource, 2);
                assert_eq!(moved, 2);
                (bag.extract(host, tx, 8, &resource, moved) == moved)
                    .then_some(())
                    .ok_or(())
            })
        };
        let deposit = |host: &mut FakeHost| {
            host.transaction(|host, tx| {
                let moved = bag.insert(host, tx, 0, &resource, 2);
                (moved == 0 || source.extract(host, tx, &resource, moved) == moved)
                    .then_some(moved)
                    .ok_or(())
            })
        };
        assert_eq!(withdraw(&mut host), Err(()));
        assert_eq!(deposit(&mut host), Ok(0));
        assert_eq!(source.stack(&mut host), Some(diamond.clone()));
        assert_eq!(destination.stack(&mut host), None);
        assert!(GlobalBackpacks.load(&mut host, ID).is_err());

        host.frozen_player_handovers.remove(&OWNER);
        assert_eq!(GlobalBackpacks.load(&mut host, ID).unwrap(), Some(saved));
        assert_eq!(withdraw(&mut host), Ok(()));
        assert_eq!(deposit(&mut host), Ok(2));
        assert_eq!(source.stack(&mut host), None);
        assert_eq!(destination.stack(&mut host), Some(diamond));
        assert_eq!(bag.amount(&mut host, 8), 0);
        assert_eq!(bag.amount(&mut host, 0), 2);
    }

    #[test]
    fn committed_generations_keep_bags_and_allocations_separate_from_old_records() {
        let mut host = fixture();
        host.join(RECIPIENT, 7);
        let borrower = (&host as &dyn Host).player(RECIPIENT).unwrap();
        let diamond = material(&host, "minecraft:diamond", 2);
        let record = Backpack {
            size: 9,
            slots: BTreeMap::from([(8, diamond.clone())]),
            paper: false,
        }
        .to_nbt(&host)
        .unwrap();
        let unrelated = BackpackId {
            owner: OWNER + 1,
            number: ID.number,
        };
        host.transaction(|host, tx| {
            GlobalBackpacks.save(host, tx, ID, record.clone())?;
            GlobalBackpacks.save(host, tx, BackpackId { number: 70, ..ID }, record.clone())?;
            GlobalBackpacks.save(host, tx, unrelated, record.clone())
        })
        .unwrap();
        recover_lost(
            &mut host,
            borrower,
            PermissionLevel::Gamemasters,
            OWNER_UUID,
            "3",
        )
        .unwrap();

        let held = ItemPlace::PlayerHand {
            player: RECIPIENT,
            hand: Hand::Main,
        };
        let old_handle = BackpackInventory {
            at: held.clone(),
            id: ID,
            generation: None,
            size: 9,
            cooler: false,
        };
        assert_eq!(old_handle.amount(&mut host, 8), 2);
        let section = Identifier::parse("slimefun:profile").unwrap();
        let snapshot = |numbers: &[u32]| {
            crate::pm_profile::Profile {
                backpacks: numbers
                    .iter()
                    .map(|&number| (number, vec![std::borrow::Cow::Borrowed(&[][..]); 9]))
                    .collect(),
                ..Default::default()
            }
            .encode(OWNER)
            .unwrap()
        };

        // The owner need not be online for a borrower to see the exact committed view.
        host.transfer_profiles
            .insert((OWNER, section.clone()), (41, snapshot(&[ID.number, 14])));
        assert_eq!(old_handle.size(&mut host), 0);
        assert!(load_bag(&mut host, ID).unwrap().unwrap().slots.is_empty());
        assert_eq!(next_number(&mut host, OWNER, Some(14)).unwrap(), 15);
        let missing = BackpackId { number: 70, ..ID };
        assert!(load_bag(&mut host, missing).unwrap().is_none());
        assert!(
            load_paper(&host, missing, None)
                .unwrap_err()
                .contains("当前的同步档案")
        );

        let active = BackpackInventory {
            at: held,
            id: ID,
            generation: Some(41),
            size: 9,
            cooler: false,
        };
        assert_eq!(
            host.transaction(|host, tx| Ok::<_, ()>(active.insert(
                host,
                tx,
                8,
                &ItemResource::of(&diamond),
                2
            ))),
            Ok(2)
        );
        host.attachments = mod_api::Attachments::from_nbt(host.attachments.to_nbt());
        assert_eq!(active.amount(&mut host, 8), 2);

        host.transfer_profiles
            .insert((OWNER, section), (42, snapshot(&[ID.number])));
        assert_eq!(active.size(&mut host), 0);
        assert!(load_bag(&mut host, ID).unwrap().unwrap().slots.is_empty());
        assert_eq!(next_number(&mut host, OWNER, Some(ID.number)).unwrap(), 4);
        assert_eq!(
            GlobalBackpacks.load(&mut host, unrelated).unwrap(),
            Some(record.clone())
        );
        host.transfer_profile_refusals
            .insert(OWNER, mod_api::TransferProfileReadError::Away);
        assert!(GlobalBackpacks.load(&mut host, ID).is_err());
        assert!(
            host.transaction(|host, tx| GlobalBackpacks.save(host, tx, ID, record))
                .is_err()
        );
    }

    #[test]
    fn lost_backpack_command_issues_the_same_live_id_and_tracks_its_size() {
        let mut host = fixture();
        host.join(RECIPIENT, 7);
        let operator = (&host as &dyn Host).player(RECIPIENT).unwrap();
        let diamond = material(&host, "minecraft:diamond", 2);
        let record = Backpack {
            size: 9,
            slots: BTreeMap::from([(8, diamond.clone())]),
            paper: false,
        };
        let saved = record.to_nbt(&host).unwrap();
        host.transaction(|host, tx| GlobalBackpacks.save(host, tx, ID, saved))
            .unwrap();

        assert!(recover_lost(&mut host, operator, PermissionLevel::All, OWNER_UUID, "3").is_err());
        assert!(
            recover_lost(
                &mut host,
                operator,
                PermissionLevel::Gamemasters,
                OWNER_UUID,
                "-3"
            )
            .is_err()
        );
        assert!(host.players.get(RECIPIENT).unwrap().slots.is_empty());

        recover_lost(
            &mut host,
            operator,
            PermissionLevel::Gamemasters,
            OWNER_UUID,
            "03",
        )
        .unwrap();
        let head = host.players.get(RECIPIENT).unwrap().slots[&0].clone();
        assert_eq!(item_id(&head).as_deref(), Some("RESTORED_BACKPACK"));
        assert_eq!(restored_id(&host, &head), Ok(ID));
        assert!(held_id(&host, &head).is_err());
        assert_eq!(held_record(&mut host, &head), Some((ID, record)));

        let slots = &mut host.players.get_mut(RECIPIENT).unwrap().slots;
        slots.remove(&0);
        slots.insert(12, head.clone());
        recover_lost(
            &mut host,
            operator,
            PermissionLevel::Gamemasters,
            OWNER_UUID,
            "3",
        )
        .unwrap();
        let slots = &host.players.get(RECIPIENT).unwrap().slots;
        assert!(!slots.contains_key(&0));
        assert_eq!(slots[&12].count(), 2);
        let stacked = slots[&12].clone();
        assert!(held_record(&mut host, &stacked).is_none());

        host.transaction(|host, tx| upgrade_bound(host, tx, ID, 9, 18))
            .unwrap();
        let paper_token = bag(&host, "RESTORED_BACKPACK", vec![string(&ID.lore())]);
        for token in [head, paper_token] {
            let (id, bag) = held_record(&mut host, &token).unwrap();
            assert_eq!(id, ID);
            assert_eq!(bag.size, 18);
            assert_eq!(bag.slots[&8], diamond);
        }
        let blank = RESTORED_BACKPACK.stack(&host, 1).unwrap();
        assert!(held_record(&mut host, &blank).is_none());
    }

    #[test]
    fn lost_paper_recovery_rolls_back_a_full_inventory_and_never_refills() {
        let mut host = fixture();
        host.join(RECIPIENT, 7);
        let operator = (&host as &dyn Host).player(RECIPIENT).unwrap();
        let diamond = material(&host, "minecraft:diamond", 2);
        let archive = |_: &dyn ServerHost, id, size| {
            assert_eq!(id, ID);
            assert_eq!(size, None);
            Ok(Backpack {
                size: 27,
                slots: BTreeMap::from([(26, diamond.clone())]),
                paper: true,
            })
        };
        assert!(
            issue_recovered(&mut host, operator, ID, |_, _, _| Err(
                "The owner has no such backpack record.".into()
            ))
            .is_err()
        );
        assert!(host.players.get(RECIPIENT).unwrap().slots.is_empty());

        let slots = &mut host.players.get_mut(RECIPIENT).unwrap().slots;
        slots.extend((0..36).map(|slot| (slot, diamond.clone())));
        let mut full = RESTORED_BACKPACK.stack(&host, 64).unwrap();
        assign_item_id(&mut full, ID).unwrap();
        host.players
            .get_mut(RECIPIENT)
            .unwrap()
            .slots
            .insert(12, full);
        assert!(issue_recovered(&mut host, operator, ID, archive).is_err());
        assert!(load_bag(&mut host, ID).unwrap().is_none());
        assert_eq!(host.players.get(RECIPIENT).unwrap().slots.len(), 36);
        assert!(host.players.get(RECIPIENT).unwrap().given.is_empty());

        host.players.get_mut(RECIPIENT).unwrap().slots.remove(&0);
        issue_recovered(&mut host, operator, ID, archive).unwrap();
        let inventory = BackpackInventory {
            at: ItemPlace::PlayerHand {
                player: RECIPIENT,
                hand: Hand::Main,
            },
            id: ID,
            generation: None,
            size: 27,
            cooler: false,
        };
        assert_eq!(inventory.size(&mut host), 27);
        host.transaction(|host, tx| {
            assert_eq!(
                inventory.extract(host, tx, 26, &ItemResource::of(&diamond), 2),
                2
            );
            Ok::<_, ()>(())
        })
        .unwrap();
        host.players.get_mut(RECIPIENT).unwrap().slots.remove(&0);
        issue_recovered(&mut host, operator, ID, |_, _, _| {
            panic!("keep the live record")
        })
        .unwrap();
        assert_eq!(inventory.size(&mut host), 27);
        assert_eq!(inventory.amount(&mut host, 26), 0);
    }

    #[test]
    fn first_open_binds_the_held_item_after_every_reserved_paper_id() {
        let mut host = fixture();
        let blank = SMALL_BACKPACK.stack(&host, 1).unwrap();
        assert_eq!(binding(&blank), Ok(Binding::Blank));
        let place = ItemPlace::PlayerHand {
            player: OWNER,
            hand: Hand::Main,
        };
        host.join(OWNER, 6).slots.insert(0, blank);
        let (first, size) = initialize_held(&mut host, &place, OWNER, Some(11)).unwrap();
        assert_eq!(size, 9);
        assert_eq!(
            first,
            BackpackId {
                owner: OWNER,
                number: 12
            }
        );
        let bound = place.stack(&mut host).unwrap();
        assert_eq!(held_id(&host, &bound), Ok((first, 9)));
        assert!(
            live_bag(&mut host, first, size)
                .unwrap()
                .unwrap()
                .slots
                .is_empty()
        );
        assert!(initialize_held(&mut host, &place, OWNER, Some(11)).is_err());
        assert_eq!(next_number(&mut host, OWNER, Some(11)), Ok(13));
        assert!(next_number(&mut host, OWNER, Some(i32::MAX as u32)).is_err());

        // Dropping or handing over the physical item leaves the original ID valid.
        host.players.get_mut(OWNER).unwrap().slots.remove(&0);
        host.join(RECIPIENT, 7).slots.insert(-106, bound);
        let off_hand = ItemPlace::PlayerHand {
            player: RECIPIENT,
            hand: Hand::Off,
        };
        let transferred = off_hand.stack(&mut host).unwrap();
        assert_eq!(held_id(&host, &transferred), Ok((first, 9)));
        assert!(GlobalBackpacks.load(&mut host, first).unwrap().is_some());
    }

    #[test]
    fn magic_workbench_keeps_the_paper_special_backpack_shapes_and_research() {
        let recipe = |id| {
            crate::recipes::magic_crafting()
                .find(|recipe| recipe.output.kind.id() == id)
                .unwrap()
        };
        let ender = recipe("ENDER_BACKPACK");
        assert_eq!(ender.research.id, "ender_backpack");
        assert_eq!(ender.research.cost, 4);
        assert_eq!(ender.output.count, 1);
        assert_eq!(
            ender.inputs.map(|kind| kind.map(Kind::id)),
            [
                Some("ENDER_LUMP_2"),
                Some("minecraft:leather"),
                Some("ENDER_LUMP_2"),
                Some("minecraft:leather"),
                Some("minecraft:chest"),
                Some("minecraft:leather"),
                Some("ENDER_LUMP_2"),
                Some("minecraft:leather"),
                Some("ENDER_LUMP_2"),
            ]
        );
        let bound = recipe("BOUND_BACKPACK");
        assert_eq!(bound.research.id, "bound_backpack");
        assert_eq!(bound.research.cost, 22);
        assert_eq!(bound.output.count, 1);
        assert_eq!(
            bound.inputs.map(|kind| kind.map(Kind::id)),
            [
                Some("ENDER_LUMP_2"),
                None,
                Some("ENDER_LUMP_2"),
                Some("ESSENCE_OF_AFTERLIFE"),
                Some("WOVEN_BACKPACK"),
                Some("ESSENCE_OF_AFTERLIFE"),
                Some("ENDER_LUMP_2"),
                None,
                Some("ENDER_LUMP_2"),
            ]
        );
        assert_eq!(recipe("NECROTIC_SKULL").research.id, "essence_of_afterlife");
        assert!(crate::recipes::crafting().all(|recipe| {
            !matches!(recipe.output.kind.id(), "BOUND_BACKPACK" | "ENDER_BACKPACK")
        }));
    }

    #[test]
    fn binding_a_woven_pack_keeps_its_paper_id_contents_capacity_and_rune() {
        let mut host = fixture();
        host.join(RECIPIENT, 7);
        let player = (&host as &dyn Host).player(RECIPIENT).unwrap();
        let diamond = material(&host, "minecraft:diamond", 2);
        let mut lore = tagged_lines();
        lore.push(string("§7Soulbound"));
        let mut inputs = [const { None }; 9];
        inputs[4] = Some(bag(&host, "WOVEN_BACKPACK", lore));
        let mut output = special::BOUND_BACKPACK.stack(&host, 1).unwrap();
        assert!(
            host.transaction(|host, tx| prepare_craft(host, tx, player, &inputs, &mut output))
                .is_err()
        );
        host.transaction(|host, tx| {
            bind_paper(host, tx, &GlobalBackpacks, ID, Some(36), |_, _, _| {
                Ok(Backpack {
                    size: 36,
                    paper: true,
                    slots: BTreeMap::from([(35, diamond.clone())]),
                })
            })
        })
        .unwrap();
        let previous = host.attachments.to_nbt();
        host.transaction(|host, tx| prepare_craft(host, tx, player, &inputs, &mut output))
            .unwrap();
        assert_eq!(held_id(&host, &output), Ok((ID, 36)));
        assert_eq!(soulbound_marker(&output), Some(Tag::Byte(1)));
        assert!(keep_on_death(&host, &output));
        assert_eq!(
            live_bag(&mut host, ID, 36).unwrap().unwrap().slots[&35],
            diamond
        );
        assert_eq!(host.attachments.to_nbt(), previous);

        let mut ender = special::ENDER_BACKPACK.stack(&host, 1).unwrap();
        let portable = ender.clone();
        host.transaction(|host, tx| prepare_craft(host, tx, player, &inputs, &mut ender))
            .unwrap();
        assert_eq!(ender, portable);
        assert_eq!(host.attachments.to_nbt(), previous);
    }

    #[test]
    fn ordinary_upgrade_keeps_the_previous_uuid_slots_and_soulbound_marker() {
        let mut host = fixture();
        host.join(RECIPIENT, 7);
        let player = (&host as &dyn Host).player(RECIPIENT).unwrap();
        let diamond = material(&host, "minecraft:diamond", 2);
        let mut lore = tagged_lines();
        lore.push(string("§7Soulbound"));
        let source = bag(&host, "SMALL_BACKPACK", lore);
        let mut inputs = [const { None }; 9];
        inputs[4] = Some(source);
        let mut output = MEDIUM_BACKPACK.stack(&host, 1).unwrap();
        assert!(
            host.transaction(|host, tx| prepare_craft(host, tx, player, &inputs, &mut output))
                .is_err()
        );
        assert!(GlobalBackpacks.load(&mut host, ID).unwrap().is_none());
        assert_eq!(binding(&output), Ok(Binding::Blank));

        host.transaction(|host, tx| {
            bind_paper(host, tx, &GlobalBackpacks, ID, Some(9), |_, _, _| {
                Ok(Backpack {
                    size: 9,
                    paper: true,
                    slots: BTreeMap::from([(8, diamond.clone())]),
                })
            })
        })
        .unwrap();
        let failed = host.transaction(|host, tx| {
            let mut output = MEDIUM_BACKPACK.stack(host, 1).unwrap();
            prepare_craft(host, tx, player, &inputs, &mut output).unwrap();
            Err::<(), _>("no space for the result")
        });
        assert!(failed.is_err());
        assert!(live_bag(&mut host, ID, 18).is_err());
        assert_eq!(
            live_bag(&mut host, ID, 9).unwrap().unwrap().slots[&8],
            diamond
        );

        host.transaction(|host, tx| prepare_craft(host, tx, player, &inputs, &mut output))
            .unwrap();
        assert_eq!(held_id(&host, &output), Ok((ID, 18)));
        assert_eq!(
            live_bag(&mut host, ID, 18).unwrap().unwrap().slots[&8],
            diamond
        );
        assert!(live_bag(&mut host, ID, 9).is_err());
        assert_eq!(
            output
                .components()
                .unwrap()
                .as_nbt()
                .get(&"minecraft:custom_data".into())
                .unwrap()
                .get("PublicBukkitValues")
                .unwrap()
                .get("slimefun:soulbound"),
            Some(&Tag::Byte(1))
        );
        assert!(
            host.transaction(|host, tx| bind_paper(
                host,
                tx,
                &GlobalBackpacks,
                ID,
                Some(9),
                |_, _, _| panic!("an old item may not refill or downsize the upgraded record")
            ))
            .is_err()
        );
    }

    #[test]
    fn crafting_chain_preserves_the_paper_tier_and_component_requirements() {
        let expected = [
            "minecraft:chest",
            "SMALL_BACKPACK",
            "MEDIUM_BACKPACK",
            "LARGE_BACKPACK",
            "WOVEN_BACKPACK",
            "GILDED_BACKPACK",
        ];
        assert_eq!(CRAFTING[0].output.count, 8);
        assert_eq!(CRAFTING[0].inputs[0].unwrap().id(), "minecraft:white_wool");
        let tiers = CRAFTING[1..]
            .iter()
            .filter(|recipe| recipe.output.kind != COOLER);
        assert_eq!(tiers.clone().count(), expected.len());
        for (index, (center, recipe)) in expected.into_iter().zip(tiers).enumerate() {
            assert_eq!(recipe.inputs[4].unwrap().id(), center);
            assert_eq!(
                tier(recipe.output.kind.id()).map(|(size, _)| size),
                Some(9 * (index as u32 + 1))
            );
        }
    }

    #[test]
    fn paper_identity_uses_uuid_and_number_without_accepting_ambiguous_lore() {
        let host = fixture();
        let mut current = bag(&host, "SMALL_BACKPACK", tagged_lines());
        assert_eq!(held_id(&host, &current), Ok((ID, 9)));
        assert_eq!(
            (0..=5)
                .zip([
                    "SMALL_BACKPACK",
                    "MEDIUM_BACKPACK",
                    "LARGE_BACKPACK",
                    "WOVEN_BACKPACK",
                    "GILDED_BACKPACK",
                    "RADIANT_BACKPACK"
                ])
                .map(|(i, id)| tier(id).map(|(size, _)| size) == Some(9 * (i + 1)))
                .collect::<Vec<_>>(),
            [true; 6],
        );
        current.set_count(2).unwrap();
        assert!(held_id(&host, &current).is_err());
        assert_eq!(
            held_id(&host, &bag(&host, "BOUND_BACKPACK", tagged_lines())),
            Ok((ID, 36))
        );
        assert!(
            held_id(
                &host,
                &bag(&host, "SMALL_BACKPACK", vec![string("ID: <ID>")])
            )
            .is_err()
        );
        let mut duplicate = tagged_lines();
        duplicate.push(string(&format!("§7ID: {OWNER_UUID}#3")));
        assert!(held_id(&host, &bag(&host, "SMALL_BACKPACK", duplicate)).is_err());
        for malformed in ["-1", "01", "2147483648", "3#4"] {
            let line = string(&format!("ID: {OWNER_UUID}#{malformed}"));
            assert!(held_id(&host, &bag(&host, "SMALL_BACKPACK", vec![line])).is_err());
        }
        assert!(may_restore(ID, OWNER, PermissionLevel::All));
        assert!(may_restore(ID, RECIPIENT, PermissionLevel::Gamemasters));
        assert!(!may_restore(ID, RECIPIENT, PermissionLevel::Moderators));
    }

    #[test]
    fn paper_bound_backpacks_keep_the_original_identity_and_recognize_legacy_runes() {
        let mut host = fixture();
        let bound = bag(&host, "BOUND_BACKPACK", tagged_lines());
        assert!(keep_on_death(&host, &bound));
        assert_eq!(held_id(&host, &bound), Ok((ID, 36)));
        assert!(!keep_on_death(
            &host,
            &bag(&host, "SMALL_BACKPACK", tagged_lines())
        ));

        let mut marked = tagged_lines();
        marked.push(string("§7Soulbound"));
        assert!(keep_on_death(
            &host,
            &bag(&host, "SMALL_BACKPACK", marked.clone())
        ));
        assert!(!keep_on_death(&host, &bag(&host, "UNKNOWN_HEAD", marked)));

        let item = material(&host, "minecraft:diamond", 2);
        let record = Backpack {
            size: 36,
            slots: BTreeMap::from([(35, item.clone())]),
            paper: true,
        };
        host.transaction(|host, tx| {
            bind_paper(host, tx, &GlobalBackpacks, ID, Some(36), |_, _, _| {
                Ok(record)
            })
        })
        .unwrap();
        assert_eq!(
            live_bag(&mut host, ID, 36).unwrap().unwrap().slots[&35],
            item
        );
    }

    #[test]
    fn archive_checks_owner_shape_slot_count_and_rejects_ambiguous_profiles() {
        let host = fixture();
        let row = "[[backpacks]]\nid = 3\nsize = 9\ninventory_slots = 0\npaper_items = []\n";
        let header =
            format!("format = 1\nplayer = '{OWNER:032x}'\npaper_profile = 'original yaml'\n");
        let valid: toml::Table = format!("{header}{row}").parse().unwrap();
        let (found, size) = archived_backpack(&valid, ID, None).unwrap();
        assert_eq!(size, 9);
        assert!(
            decode_archived_slots(&host, found, size)
                .unwrap()
                .is_empty()
        );
        assert!(archived_backpack(&valid, ID, Some(18)).is_err());
        assert!(
            archived_backpack(
                &valid,
                BackpackId {
                    owner: RECIPIENT,
                    ..ID
                },
                Some(9)
            )
            .is_err()
        );
        let repeated: toml::Table = format!("{header}{row}{row}").parse().unwrap();
        assert!(
            archived_backpack(&repeated, ID, Some(9))
                .unwrap_err()
                .contains("repeats")
        );
        let ambiguous: toml::Table =
            format!("{header}paper_profile_issues=['duplicated alias']\n{row}")
                .parse()
                .unwrap();
        assert!(
            archived_backpack(&ambiguous, ID, Some(9))
                .unwrap_err()
                .contains("duplicated alias")
        );
        let out_of_bounds: toml::Table = "inventory_slots=1\npaper_items=[{ slot=9, id='minecraft:diamond', data_version=4903, count=1, components={} }]".parse().unwrap();
        assert!(
            decode_archived_slots(&host, &out_of_bounds, 9)
                .unwrap_err()
                .contains("slot")
        );
    }

    #[test]
    fn transferred_bag_uses_the_origins_canonical_slots_and_retries_cannot_refill() {
        let mut host = fixture();
        let held = bag(&host, "SMALL_BACKPACK", tagged_lines());
        let shell = ItemPlace::PlayerHand {
            player: OWNER,
            hand: Hand::Main,
        };
        host.join(OWNER, 6).slots.insert(0, held.clone());
        host.join(RECIPIENT, 7);
        let mut diamond = material(&host, "minecraft:diamond", 2);
        diamond
            .set_components(Compound::from([(
                "minecraft:custom_name".into(),
                string("Saved Diamond"),
            )]))
            .unwrap();
        let input = || {
            Ok(Backpack {
                size: 9,
                paper: true,
                slots: BTreeMap::from([(8, diamond.clone())]),
            })
        };
        let declined = host.transaction(|host, tx| {
            assert_eq!(
                bind_paper(host, tx, &GlobalBackpacks, ID, Some(9), |_, _, _| input()),
                Ok(Restored::Installed)
            );
            Err::<(), _>(())
        });
        assert!(declined.is_err());
        assert!(GlobalBackpacks.load(&mut host, ID).unwrap().is_none());

        host.transaction(|host, tx| {
            bind_paper(host, tx, &GlobalBackpacks, ID, Some(9), |_, _, _| input())
        })
        .unwrap();
        host.attachments = mod_api::Attachments::from_nbt(host.attachments.to_nbt());
        assert_eq!(
            live_bag(&mut host, ID, 9).unwrap().unwrap().slots.get(&8),
            Some(&diamond)
        );
        let original = BackpackInventory {
            at: shell,
            id: ID,
            generation: None,
            size: 9,
            cooler: false,
        };
        assert_eq!(original.amount(&mut host, 8), 2);
        host.players.get_mut(OWNER).unwrap().slots.remove(&0);
        host.players
            .get_mut(RECIPIENT)
            .unwrap()
            .slots
            .insert(0, held.clone());
        assert_eq!(original.size(&mut host), 0);

        let transferred = BackpackInventory {
            at: ItemPlace::PlayerHand {
                player: RECIPIENT,
                hand: Hand::Main,
            },
            id: ID,
            generation: None,
            size: 9,
            cooler: false,
        };
        assert_eq!(transferred.amount(&mut host, 8), 2);
        let recipient = (&host as &dyn Host).player(RECIPIENT).unwrap();
        assert!(
            restore_held(
                &mut host,
                &GlobalBackpacks,
                recipient,
                Hand::Main,
                PermissionLevel::All
            )
            .unwrap_err()
            .contains("owner")
        );
        let boxed = material(&host, "minecraft:red_shulker_box", 1);
        host.transaction(|host, tx| {
            assert_eq!(
                transferred.insert(host, tx, 1, &ItemResource::of(&boxed), 1),
                0
            );
            assert_eq!(
                transferred.insert(host, tx, 1, &ItemResource::of(&held), 1),
                0
            );
            assert_eq!(
                transferred.extract(host, tx, 8, &ItemResource::of(&diamond), 2),
                2
            );
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(
            host.transaction(|host, tx| bind_paper(
                host,
                tx,
                &GlobalBackpacks,
                ID,
                Some(9),
                |_, _, _| panic!("already imported")
            )),
            Ok(Restored::AlreadyInstalled)
        );
        assert_eq!(transferred.amount(&mut host, 8), 0);
    }
}
