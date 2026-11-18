// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use mod_api::{
    AttachmentCodec, AttachmentType, ComponentId, Compound, ConfigFileError, ConfigPath, Holder,
    Host, HostTransaction, ItemResource, ItemStack, MenuClosedEvent, MenuReplace, MenuSlotPlace,
    PlaceProvider, PlayerRef, Registrar, ServerHost, Tag, Text, Transaction,
};

use crate::storage::{decode_archived_slots, encoded_dimension, validate_inventory_preset};

const PROVIDER: &str = "slimefun:universal_storage";
const MAX_SLOTS: u32 = 54;
const SLOT_BITS: u32 = 6;

static INVENTORIES: AttachmentType<Compound> = AttachmentType {
    namespace: "slimefun",
    name: "universal_storage",
    default: Compound::new,
    codec: Some(AttachmentCodec {
        encode: |value| Ok(value.clone()),
        decode: |value| Ok(value.clone()),
    }),
    copy_on_death: false,
    sync: false,
};

#[derive(Debug, PartialEq, Eq)]
struct Layout {
    title: String,
    size: u32,
    slots: u64,
}

impl Layout {
    fn has(&self, slot: u32) -> bool {
        slot < self.size && self.slots & (1 << slot) != 0
    }

    fn valid(&self) -> bool {
        (9..=MAX_SLOTS).contains(&self.size)
            && self.size.is_multiple_of(9)
            && self.slots != 0
            && self.slots >> self.size == 0
            && !self.title.trim().is_empty()
            && self.title.len() <= 256
            && !self.title.chars().any(char::is_control)
    }
}

fn mask(values: &toml::Value, capacity: u32) -> Result<u64, &'static str> {
    values
        .as_array()
        .ok_or("Expected a list of menu slots.")?
        .iter()
        .try_fold(0, |mask, value| {
            let slot = value
                .as_integer()
                .and_then(|value| u32::try_from(value).ok())
                .filter(|&slot| slot < capacity)
                .ok_or("A menu slot is outside the inventory.")?;
            let bit = 1u64 << slot;
            if mask & bit != 0 {
                Err("The menu repeats a slot.")
            } else {
                Ok(mask | bit)
            }
        })
}

fn preset_path(preset: &str) -> Result<String, String> {
    if preset.trim().is_empty() || preset.len() > 120 || preset.chars().any(char::is_control) {
        return Err("Invalid Paper universal inventory preset name.".into());
    }
    Ok(encoded_dimension(preset))
}

fn read(host: &dyn ServerHost, path: &str) -> Result<Option<toml::Table>, String> {
    let path = ConfigPath::new(path).map_err(|_| "Invalid Paper universal inventory path.")?;
    let bytes = match host.world_data_files().read(&path) {
        Ok(bytes) => bytes,
        Err(ConfigFileError::NotFound) => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Could not read the Paper universal inventory: {error}"
            ));
        }
    };
    let data =
        std::str::from_utf8(&bytes).map_err(|_| "The Paper universal inventory must be UTF-8.")?;
    data.parse()
        .map(Some)
        .map_err(|_| "The Paper universal inventory has invalid TOML.".into())
}

fn first_layout(
    preset: &str,
    archive: &toml::Table,
    explicit: Option<&toml::Table>,
    occupied: u64,
) -> Result<Layout, String> {
    let layout = if let Some(explicit) = explicit {
        if explicit.get("preset").and_then(toml::Value::as_str) != Some(preset) {
            return Err("The Paper layout belongs to a different preset.".into());
        }
        let size = explicit
            .get("size")
            .and_then(toml::Value::as_integer)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|&size| size <= MAX_SLOTS)
            .ok_or("Invalid Paper menu size.")?;
        let slots = mask(
            explicit
                .get("slots")
                .ok_or("The Paper menu layout has no slots.")?,
            size,
        )?;
        let title = explicit
            .get("title")
            .and_then(toml::Value::as_str)
            .ok_or("The Paper menu layout has no title.")?;
        Layout {
            title: title.to_owned(),
            size,
            slots,
        }
    } else {
        let slots = match archive.get("paper_slots") {
            Some(values) => mask(values, MAX_SLOTS)?,
            None => occupied,
        };
        Layout {
            title: format!("Paper {preset}"),
            size: MAX_SLOTS,
            slots,
        }
    };
    if !layout.valid() || occupied & !layout.slots != 0 {
        return Err("The Paper layout must expose all archived slots. An empty archive needs its original layout.".into());
    }
    Ok(layout)
}

struct Inventory {
    layout: Layout,
    items: BTreeMap<u32, ItemStack>,
}

impl Inventory {
    fn decode(host: &dyn Host, data: &Compound) -> Result<Self, String> {
        let (
            Some(Tag::Int(size)),
            Some(Tag::Long(slots)),
            Some(Tag::String(title)),
            Some(Tag::Compound(items)),
        ) = (
            data.get(&"size".into()),
            data.get(&"slots".into()),
            data.get(&"title".into()),
            data.get(&"items".into()),
        )
        else {
            return Err("The saved universal inventory is corrupt.".into());
        };
        let layout = Layout {
            size: u32::try_from(*size).map_err(|_| "Invalid universal inventory size.")?,
            slots: u64::try_from(*slots).map_err(|_| "Invalid universal inventory slots.")?,
            title: title
                .to_string()
                .map_err(|_| "Invalid universal inventory title.")?,
        };
        if !layout.valid() {
            return Err("The saved universal layout is corrupt.".into());
        }
        let mut stacks = BTreeMap::new();
        for (slot, stack) in items {
            let slot = slot
                .to_string()
                .ok()
                .and_then(|text| {
                    let index = text.parse::<u32>().ok()?;
                    (index.to_string() == text).then_some(index)
                })
                .filter(|&slot| layout.has(slot))
                .ok_or("The saved universal slot is invalid.")?;
            let Tag::Compound(stack) = stack else {
                return Err("A saved universal item is invalid.".into());
            };
            let stack = ItemStack::from_nbt(host, stack)
                .ok_or("A saved universal item could not be decoded.")?;
            stacks.insert(slot, stack);
        }
        Ok(Self {
            layout,
            items: stacks,
        })
    }

    fn encode(&self, host: &dyn Host) -> Result<Compound, String> {
        let mut items = Compound::new();
        if !self.layout.valid() {
            return Err("Invalid universal inventory layout.".into());
        }
        for (&slot, stack) in &self.items {
            if !self.layout.has(slot) {
                return Err("This universal inventory does not have that slot.".into());
            }
            let stack = stack
                .to_nbt(host)
                .ok_or("The universal item could not be encoded.")?;
            items.insert(slot.to_string().as_str().into(), Tag::Compound(stack));
        }
        Ok(Compound::from([
            ("size".into(), Tag::Int(self.layout.size as i32)),
            ("slots".into(), Tag::Long(self.layout.slots as i64)),
            (
                "title".into(),
                Tag::String(self.layout.title.as_str().into()),
            ),
            ("items".into(), Tag::Compound(items)),
        ]))
    }
}

fn live(host: &mut dyn ServerHost, preset: &str) -> Result<Option<Inventory>, String> {
    let lookup = |data: &Compound| data.get(&preset.into()).cloned();
    let value = match INVENTORIES
        .with_decoded(host, Holder::Server, |data| data.map(lookup))
        .map_err(|_| "The global universal inventory store is unavailable.")?
    {
        Some(value) => value,
        None => INVENTORIES
            .get_existing(host, Holder::Server)
            .map_err(|_| "The global universal inventory store is unavailable.")?
            .as_ref()
            .and_then(lookup),
    };
    match value {
        Some(Tag::Compound(value)) => Inventory::decode(host, &value).map(Some),
        Some(_) => Err("The saved universal inventory is corrupt.".into()),
        None => Ok(None),
    }
}

fn save(host: &mut dyn ServerHost, preset: &str, inventory: &Inventory) -> Result<(), String> {
    let value = inventory.encode(host)?;
    let mut records = INVENTORIES
        .get_existing(host, Holder::Server)
        .map_err(|_| "The global universal inventory store is unavailable.")?
        .unwrap_or_default();
    records.insert(preset.into(), Tag::Compound(value));
    INVENTORIES
        .set(host, Holder::Server, records)
        .map_err(|_| "The universal inventory could not be saved.".into())
}

fn restore_once(
    host: &mut dyn ServerHost,
    preset: &str,
    prepare: impl FnOnce(&dyn ServerHost) -> Result<Inventory, String>,
) -> Result<bool, String> {
    host.transaction(|host, _| {
        if live(host, preset)?.is_some() {
            return Ok(false);
        }
        let inventory = prepare(host)?;
        save(host, preset, &inventory)?;
        Ok(true)
    })
}

pub(crate) fn restore(host: &mut dyn ServerHost, preset: &str) -> Result<bool, String> {
    let path = preset_path(preset)?;
    restore_once(host, preset, |host| {
        let archive = read(host, &format!("storage-import/universal/{path}.toml"))?
            .ok_or("No Paper universal inventory exists for that exact preset.")?;
        if archive.get("format").and_then(toml::Value::as_integer) != Some(1)
            || archive.get("preset").and_then(toml::Value::as_str) != Some(preset)
            || archive
                .get("paper_inventory")
                .and_then(toml::Value::as_str)
                .is_none()
        {
            return Err("The archive does not identify this Paper universal inventory.".into());
        }
        validate_inventory_preset(&archive, preset)?;
        let items = decode_archived_slots(host, &archive, MAX_SLOTS)?;
        let occupied = items.keys().fold(0, |mask, &slot| mask | (1 << slot));
        let explicit = read(host, &format!("universal-layout/{path}.toml"))?;
        let layout = first_layout(preset, &archive, explicit.as_ref(), occupied)?;
        Ok(Inventory { layout, items })
    })
}

#[derive(Clone)]
struct View {
    player: PlayerRef,
    window: i32,
    preset: String,
}

#[derive(Default)]
struct Viewers {
    next: u64,
    open: BTreeMap<u64, View>,
}

#[derive(Clone, Default)]
pub(crate) struct UniversalStorage(Arc<Mutex<Viewers>>);

impl UniversalStorage {
    pub(crate) fn register(&self, registrar: &mut dyn Registrar) {
        registrar.place_provider(PROVIDER, Arc::new(self.clone()));
    }

    pub(crate) fn clear(&self) {
        self.0.lock().unwrap().open.clear();
    }

    pub(crate) fn closed(&self, event: &MenuClosedEvent) {
        self.0
            .lock()
            .unwrap()
            .open
            .retain(|_, view| view.player.uuid() != event.uuid || view.window != event.window);
    }

    pub(crate) fn open(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        preset: &str,
    ) -> Result<(), String> {
        preset_path(preset)?;
        if player.permission_level(host).is_none_or(|level| level < 2) {
            return Err("Opening archived universal inventories requires operator level 2.".into());
        }
        let inventory =
            live(host, preset)?.ok_or("Restore this Paper inventory before opening it.")?;
        let key = {
            let mut viewers = self.0.lock().unwrap();
            viewers.next = viewers
                .next
                .checked_add(1)
                .filter(|&id| id <= (u64::MAX >> SLOT_BITS))
                .ok_or("Universal menu keys are exhausted.")?;
            viewers.next
        };
        let window = host
            .open_menu(
                player.uuid(),
                (inventory.layout.size / 9) as u8,
                Text::literal(inventory.layout.title.clone()),
                Vec::new(),
                MenuReplace::CloseFirst,
            )
            .map_err(|_| "The universal inventory menu could not open.")?
            .ok_or("The universal inventory menu did not open.")?;
        let view = View {
            player,
            window,
            preset: preset.to_owned(),
        };
        {
            let mut viewers = self.0.lock().unwrap();
            viewers
                .open
                .retain(|_, old| old.player.uuid() != player.uuid());
            viewers.open.insert(key, view);
        }
        let places = (0..inventory.layout.size)
            .map(|slot| {
                if inventory.layout.has(slot) {
                    MenuSlotPlace::ModSlot {
                        provider: PROVIDER,
                        key: (key << SLOT_BITS) | u64::from(slot),
                    }
                } else {
                    MenuSlotPlace::Decoration
                }
            })
            .collect();
        if player.back_mod_menu_slots(host, window, places).is_err() {
            self.0.lock().unwrap().open.remove(&key);
            host.close_menu(player.uuid());
            return Err("The universal inventory slots could not be bound.".into());
        }
        Ok(())
    }

    fn target(&self, host: &mut dyn ServerHost, key: u64) -> Option<(View, Inventory, u32)> {
        let id = key >> SLOT_BITS;
        let slot = (key & ((1 << SLOT_BITS) - 1)) as u32;
        let view = self.0.lock().unwrap().open.get(&id)?.clone();
        if view
            .player
            .permission_level(host)
            .is_none_or(|level| level < 2)
            || host.menu(view.player.uuid(), view.window).is_none()
        {
            return None;
        }
        let inventory = live(host, &view.preset).ok()??;
        inventory
            .layout
            .has(slot)
            .then_some((view, inventory, slot))
    }
}

fn capacity(host: &dyn Host, resource: &ItemResource) -> u32 {
    let id = ComponentId::new("minecraft:max_stack_size").expect("built-in component");
    match resource.value(host, &id) {
        Some(Tag::Int(count @ 1..=99)) => (count as u32).min(64),
        _ => host.max_stack_size(resource.item()).unwrap_or(0).min(64),
    }
}

fn insert_slot(
    host: &dyn Host,
    slots: &mut BTreeMap<u32, ItemStack>,
    slot: u32,
    resource: &ItemResource,
    amount: u32,
) -> u32 {
    let current = slots.get(&slot);
    if current.is_some_and(|stack| !resource.matches(stack)) {
        return 0;
    }
    let count = current.map_or(0, ItemStack::count);
    let moved = capacity(host, resource).saturating_sub(count).min(amount);
    if moved == 0 {
        return 0;
    }
    let Some(stack) = resource.to_stack(count + moved) else {
        return 0;
    };
    slots.insert(slot, stack);
    moved
}

fn extract_slot(
    slots: &mut BTreeMap<u32, ItemStack>,
    slot: u32,
    resource: &ItemResource,
    amount: u32,
) -> u32 {
    let Some(stack) = slots.get_mut(&slot).filter(|stack| resource.matches(stack)) else {
        return 0;
    };
    let moved = stack.count().min(amount);
    if moved == 0 {
        return 0;
    }
    if moved == stack.count() {
        slots.remove(&slot);
    } else if stack.set_count(stack.count() - moved).is_err() {
        return 0;
    }
    moved
}

impl PlaceProvider for UniversalStorage {
    fn menu_slot_exists(&self, host: &mut dyn ServerHost, key: u64) -> bool {
        self.target(host, key).is_some()
    }

    fn menu_slot_capacity(
        &self,
        host: &mut dyn ServerHost,
        key: u64,
        resource: &ItemResource,
    ) -> u32 {
        if self.target(host, key).is_some() {
            capacity(host, resource)
        } else {
            0
        }
    }

    fn stack(&self, host: &mut dyn ServerHost, key: u64) -> Option<ItemStack> {
        let (_, mut inventory, slot) = self.target(host, key)?;
        inventory.items.remove(&slot)
    }

    fn insert(
        &self,
        host: &mut dyn ServerHost,
        _: &Transaction,
        key: u64,
        resource: &ItemResource,
        amount: u32,
    ) -> u32 {
        let Some((view, mut inventory, slot)) = self.target(host, key) else {
            return 0;
        };
        let moved = insert_slot(host, &mut inventory.items, slot, resource, amount);
        if moved > 0 && save(host, &view.preset, &inventory).is_ok() {
            moved
        } else {
            0
        }
    }

    fn extract(
        &self,
        host: &mut dyn ServerHost,
        _: &Transaction,
        key: u64,
        resource: &ItemResource,
        amount: u32,
    ) -> u32 {
        let Some((view, mut inventory, slot)) = self.target(host, key) else {
            return 0;
        };
        let moved = extract_slot(&mut inventory.items, slot, resource, amount);
        if moved > 0 && save(host, &view.preset, &inventory).is_ok() {
            moved
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn the_layout_keeps_real_paper_slots_without_inventing_unknown_slots() {
        let empty = toml::Table::new();
        let old = first_layout("SHARED", &empty, None, 1 << 8).unwrap();
        assert_eq!(old.size, 54);
        assert!(old.has(8));
        assert!(!old.has(0));
        assert!(first_layout("SHARED", &empty, None, 0).is_err());

        let archived: toml::Table = "paper_slots=[8,17]".parse().unwrap();
        let layout = first_layout("SHARED", &archived, None, 1 << 8).unwrap();
        assert!(layout.has(8) && layout.has(17) && !layout.has(16));
        let explicit: toml::Table = "preset='SHARED'\nsize=18\ntitle='Original'\nslots=[0,8,17]"
            .parse()
            .unwrap();
        let layout = first_layout("SHARED", &empty, Some(&explicit), 1 << 8).unwrap();
        assert_eq!(layout.title, "Original");
        assert!(layout.has(0) && layout.has(17) && !layout.has(18));
        assert!(first_layout("Different", &empty, Some(&explicit), 1 << 8).is_err());
        assert!(first_layout("SHARED", &empty, Some(&explicit), 1 << 12).is_err());
    }

    #[test]
    fn a_named_inventory_shares_exact_slots_after_reload_and_restore_cannot_refill_it() {
        let mut host = FakeHost::new();
        let material = Identifier::parse("minecraft:diamond").unwrap();
        host.registries.resolve_items([material.clone()]).unwrap();
        let mut item =
            ItemStack::new((&host as &dyn Host).item_key(&material).unwrap(), 2).unwrap();
        item.set_components(Compound::from([(
            "minecraft:custom_name".into(),
            Tag::String("Archived item".into()),
        )]))
        .unwrap();
        let resource = ItemResource::of(&item);
        let original = || {
            Ok(Inventory {
                layout: Layout {
                    title: "Shared".into(),
                    size: 18,
                    slots: (1 << 8) | (1 << 17),
                },
                items: BTreeMap::from([(17, item.clone())]),
            })
        };
        assert!(restore_once(&mut host, "SHARED", |_| Err("bad archive slot".into())).is_err());
        assert!(live(&mut host, "SHARED").unwrap().is_none());
        assert!(restore_once(&mut host, "SHARED", |_| original()).unwrap());
        assert!(live(&mut host, "shared").unwrap().is_none());
        let before = host.attachments.to_nbt();
        let aborted = host.transaction(|host, _| {
            let mut inventory = live(host, "SHARED").unwrap().unwrap();
            assert_eq!(extract_slot(&mut inventory.items, 17, &resource, 2), 2);
            save(host, "SHARED", &inventory).unwrap();
            Err::<(), _>(())
        });
        assert!(aborted.is_err());
        assert_eq!(host.attachments.to_nbt(), before);
        host.transaction(|host, _| {
            let mut inventory = live(host, "SHARED").unwrap().unwrap();
            assert_eq!(inventory.items.get(&17), Some(&item));
            assert_eq!(extract_slot(&mut inventory.items, 17, &resource, 2), 2);
            save(host, "SHARED", &inventory)
        })
        .unwrap();
        host.attachments = mod_api::Attachments::from_nbt(host.attachments.to_nbt());
        let active = live(&mut host, "SHARED").unwrap().unwrap();
        assert!(active.layout.has(17));
        assert!(active.items.is_empty());
        assert!(!restore_once(&mut host, "SHARED", |_| original()).unwrap());
        assert!(live(&mut host, "SHARED").unwrap().unwrap().items.is_empty());
    }
}
