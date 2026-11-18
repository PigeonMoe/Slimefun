// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
};

use mod_api::{
    AttachmentCodec, AttachmentType, ComponentId, Compound, ConfigFileError, ConfigPath, Hand,
    Holder, Host, HostTransaction, ITEM_ITEMS, ItemPlace, ItemResource, ItemStack, MenuClosedEvent,
    MenuReplace, MenuSlotPlace, PermissionLevel, PlayerRef, Registrar, ResourceHandler, ServerHost,
    Tag, Text, Transaction,
};

use crate::{items::item_id, recipes::Research, research, storage::decode_archived_slots};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct BackpackId {
    pub owner: u128,
    pub number: u32,
}

impl BackpackId {
    pub(crate) fn storage_key(self) -> String {
        format!("{:032x}#{}", self.owner, self.number)
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
const WOVEN_BACKPACK: Research = Research {
    id: "woven_backpack",
    name: "Woven Backpack",
    cost: 19,
};
const GILDED_BACKPACK: Research = Research {
    id: "gilded_backpack",
    name: "Gilded Backpack",
    cost: 22,
};
const RADIANT_BACKPACK: Research = Research {
    id: "radiant_backpack",
    name: "Radiant Backpack",
    cost: 25,
};

pub(crate) const RESEARCHES: &[Research] = &[
    BASIC_BACKPACKS,
    WOVEN_BACKPACK,
    GILDED_BACKPACK,
    RADIANT_BACKPACK,
];

fn tier(id: &str) -> Option<(u32, &'static Research)> {
    Some(match id {
        "SMALL_BACKPACK" => (9, &BASIC_BACKPACKS),
        "MEDIUM_BACKPACK" => (18, &BASIC_BACKPACKS),
        "LARGE_BACKPACK" => (27, &BASIC_BACKPACKS),
        "WOVEN_BACKPACK" => (36, &WOVEN_BACKPACK),
        "GILDED_BACKPACK" => (45, &GILDED_BACKPACK),
        "RADIANT_BACKPACK" => (54, &RADIANT_BACKPACK),
        _ => return None,
    })
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
        tier(&id).is_none()
            && !matches!(
                id.as_str(),
                "BOUND_BACKPACK" | "RESTORED_BACKPACK" | "COOLER" | "ENDER_BACKPACK"
            )
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

pub(crate) fn held_id(
    host: &dyn Host,
    stack: &ItemStack,
) -> Result<(BackpackId, u32), &'static str> {
    if stack.item().id(host) != "minecraft:player_head" {
        return Err("Hold an ordinary Slimefun backpack.");
    }
    let capacity = item_id(stack)
        .as_deref()
        .and_then(tier)
        .map(|(size, _)| size)
        .ok_or("Hold an ordinary Slimefun backpack.")?;
    if stack.count() != 1 {
        return Err("Backpacks must be held one at a time.");
    }
    match binding(stack)? {
        Binding::Named(id) => Ok((id, capacity)),
        Binding::Blank => Err("This backpack has no existing Paper ID."),
    }
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

fn archived_backpack(
    profile: &toml::Table,
    id: BackpackId,
    size: u32,
) -> Result<&toml::Table, String> {
    if profile.get("format").and_then(toml::Value::as_integer) != Some(1) {
        return Err("Unsupported archived Paper backpack format.".into());
    }
    let owner = format!("{:032x}", id.owner);
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
    let mut seen = BTreeSet::new();
    let mut found = None;
    for entry in entries {
        let entry = entry.as_table().ok_or("Invalid Paper backpack record.")?;
        let number = entry
            .get("id")
            .and_then(toml::Value::as_integer)
            .and_then(|n| i32::try_from(n).ok())
            .and_then(|n| u32::try_from(n).ok())
            .ok_or("Invalid archived Paper backpack number.")?;
        if !seen.insert(number) {
            return Err(format!(
                "The Paper profile repeats backpack number {number}."
            ));
        }
        if number == id.number {
            found = Some(entry);
        }
    }
    let entry = found.ok_or("The Paper profile does not contain this backpack number.")?;
    if !valid_size(size)
        || entry.get("size").and_then(toml::Value::as_integer) != Some(i64::from(size))
    {
        return Err("The Paper backpack size does not match the held backpack tier.".into());
    }
    Ok(entry)
}

fn load_paper(host: &dyn ServerHost, id: BackpackId, size: u32) -> Result<Backpack, String> {
    let path = ConfigPath::new(&format!("storage-import/backpacks/{:032x}.toml", id.owner))
        .expect("hexadecimal UUID is a valid archive path");
    let bytes = host
        .world_data_files()
        .read(&path)
        .map_err(|error| match error {
            ConfigFileError::NotFound => {
                "There is no Paper archive for this backpack owner.".to_owned()
            }
            _ => format!("Could not read the archived Paper backpack: {error}"),
        })?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| "The Paper backpack archive must be UTF-8.")?;
    let profile: toml::Table = text
        .parse()
        .map_err(|_| "The Paper backpack archive contains invalid TOML.")?;
    let row = archived_backpack(&profile, id, size)?;
    let slots = decode_archived_slots(host, row, size)?;
    Ok(Backpack {
        size,
        slots,
        paper: true,
    })
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

pub(crate) struct GlobalBackpacks;

impl BackpackStore for GlobalBackpacks {
    fn load(&self, host: &mut dyn ServerHost, id: BackpackId) -> Result<Option<Compound>, String> {
        let key = id.storage_key();
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
            None => Ok(None),
        }
    }

    fn save(
        &self,
        host: &mut dyn ServerHost,
        _: &Transaction,
        id: BackpackId,
        record: Compound,
    ) -> Result<(), String> {
        let mut data = RECORDS
            .get_existing(host, Holder::Server)
            .map_err(|_| "The global backpack store is unavailable.")?
            .unwrap_or_default();
        data.insert(id.storage_key().as_str().into(), Tag::Compound(record));
        RECORDS
            .set(host, Holder::Server, data)
            .map_err(|_| "Could not write the global backpack record.".into())
    }
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
    size: u32,
    load: impl FnOnce(&dyn ServerHost, BackpackId, u32) -> Result<Backpack, String>,
) -> Result<Restored, String> {
    if let Some(data) = store.load(host, id)? {
        let current = Backpack::from_nbt(host, &data)?;
        return if current.paper && current.size == size {
            Ok(Restored::AlreadyInstalled)
        } else {
            Err("A different live backpack already has this identity.".into())
        };
    }
    let imported = load(host, id, size)?;
    if !imported.paper || imported.size != size {
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
        let (id, size) = held_id(host, &stack)?;
        if !may_restore(id, player.uuid(), permission) {
            return Err(
                "The original owner or a level-2 operator must restore this backpack.".into(),
            );
        }
        bind_paper(host, tx, store, id, size, load_paper)
    })
}

fn live_bag(
    host: &mut dyn ServerHost,
    id: BackpackId,
    size: u32,
) -> Result<Option<Backpack>, String> {
    let Some(data) = GlobalBackpacks.load(host, id)? else {
        return Ok(None);
    };
    let bag = Backpack::from_nbt(host, &data)?;
    if bag.size != size {
        return Err("The live backpack size does not match the held backpack tier.".into());
    }
    Ok(Some(bag))
}

pub(crate) fn register(registrar: &mut dyn Registrar) {
    registrar.item_capability(&ITEM_ITEMS, "minecraft:player_head", |host, at| {
        if !matches!(at, ItemPlace::PlayerHand { .. }) {
            return None;
        }
        let stack = at.stack(host)?;
        let (id, size) = held_id(host, &stack).ok()?;
        live_bag(host, id, size).ok()??;
        Some(Box::new(BackpackInventory {
            at: at.clone(),
            id,
            size,
        }))
    });
}

struct BackpackInventory {
    at: ItemPlace,
    id: BackpackId,
    size: u32,
}

impl BackpackInventory {
    fn read(&self, host: &mut dyn ServerHost) -> Option<Backpack> {
        let held = self.at.stack(host)?;
        if held_id(host, &held).ok()? != (self.id, self.size) {
            return None;
        }
        live_bag(host, self.id, self.size).ok().flatten()
    }

    fn write(&self, host: &mut dyn ServerHost, tx: &Transaction, bag: &Backpack) -> bool {
        let Ok(data) = bag.to_nbt(host) else {
            return false;
        };
        GlobalBackpacks.save(host, tx, self.id, data).is_ok()
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
            || !allowed_contents(host, resource.as_stack())
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
        if slot >= bag.size || !allowed_contents(host, resource.as_stack()) {
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

#[derive(Default)]
pub(crate) struct BackpackMenus(Mutex<BTreeMap<u128, (BackpackId, i32)>>);

impl BackpackMenus {
    pub(crate) fn clear(&self) {
        self.0.lock().unwrap().clear();
    }

    pub(crate) fn closed(&self, event: &MenuClosedEvent) {
        let mut viewers = self.0.lock().unwrap();
        if viewers
            .get(&event.uuid)
            .is_some_and(|&(_, window)| event.window == window)
        {
            viewers.remove(&event.uuid);
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
        let Some((_, required)) = item_id(&stack).as_deref().and_then(tier) else {
            return false;
        };
        let result = held_id(host, &stack)
            .map_err(str::to_owned)
            .and_then(|(id, size)| {
                if !research::can_use(host, player, required) {
                    return Err(format!("Unlock this research with /sf research {}.", required.id));
                }
                if live_bag(host, id, size)?.is_none() {
                    return Err("Restore this Paper backpack with /sf restore-backpack while holding it in your main hand.".into());
                }
                {
                    let mut viewers = self.0.lock().unwrap();
                    viewers.retain(|uuid, (_, window)| host.menu(*uuid, *window).is_some());
                    if viewers
                        .iter()
                        .any(|(&uuid, &(open, _))| open == id && uuid != player.uuid())
                    {
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
                self.0.lock().unwrap().insert(player.uuid(), (id, window));
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
        assert!(held_id(&host, &bag(&host, "BOUND_BACKPACK", tagged_lines())).is_err());
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
    fn archive_checks_owner_shape_slot_count_and_rejects_ambiguous_profiles() {
        let host = fixture();
        let row = "[[backpacks]]\nid = 3\nsize = 9\ninventory_slots = 0\npaper_items = []\n";
        let header =
            format!("format = 1\nplayer = '{OWNER:032x}'\npaper_profile = 'original yaml'\n");
        let valid: toml::Table = format!("{header}{row}").parse().unwrap();
        let found = archived_backpack(&valid, ID, 9).unwrap();
        assert!(decode_archived_slots(&host, found, 9).unwrap().is_empty());
        assert!(archived_backpack(&valid, ID, 18).is_err());
        assert!(
            archived_backpack(
                &valid,
                BackpackId {
                    owner: RECIPIENT,
                    ..ID
                },
                9
            )
            .is_err()
        );
        let repeated: toml::Table = format!("{header}{row}{row}").parse().unwrap();
        assert!(
            archived_backpack(&repeated, ID, 9)
                .unwrap_err()
                .contains("repeats")
        );
        let ambiguous: toml::Table =
            format!("{header}paper_profile_issues=['duplicated alias']\n{row}")
                .parse()
                .unwrap();
        assert!(
            archived_backpack(&ambiguous, ID, 9)
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
                bind_paper(host, tx, &GlobalBackpacks, ID, 9, |_, _, _| input()),
                Ok(Restored::Installed)
            );
            Err::<(), _>(())
        });
        assert!(declined.is_err());
        assert!(GlobalBackpacks.load(&mut host, ID).unwrap().is_none());

        host.transaction(|host, tx| {
            bind_paper(host, tx, &GlobalBackpacks, ID, 9, |_, _, _| input())
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
            size: 9,
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
            size: 9,
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
                9,
                |_, _, _| panic!("already imported")
            )),
            Ok(Restored::AlreadyInstalled)
        );
        assert_eq!(transferred.amount(&mut host, 8), 0);
    }
}
