// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockDropsEvent, BlockEntityRef, BlockKey,
    BlockShapeKind, BlockState, BlockUpdateFlags, Compound, DispenserSelectedEvent, Flow, Holder,
    Host, HostSide, HostTransaction, ITEMS, InteractionResult, ItemPart, ItemResource, ItemStack,
    LevelHost, LevelRef, OnBlockPlacedByEvent, ScheduledTaskEvent, ServerHost, Tag, TaskId, Text,
    UseOnEvent,
};

use crate::{
    items::{ELECTRIC_MOTOR, GOLD_4K, Kind, OUTPUT_CHEST, SlimefunItem, item_id, text},
    network_runtime::{Networks, parse_uuid},
    networks::Component,
    recipes::{CraftingRecipe, Research, Stack},
    research, storage,
};

type Pos = [i32; 3];
const SLOTS: usize = 9;
const DELAY: u32 = 2;

pub(crate) const BLOCK_PLACER: Kind = Kind::Slimefun(&SlimefunItem {
    id: "BLOCK_PLACER",
    material: "minecraft:dispenser",
    name: "Block Placer",
    color: "green",
    tier: None,
    texture: None,
    juice: None,
});

pub(crate) const RESEARCH: Research = Research {
    id: "block_placer",
    name: "Block Placer",
    cost: 17,
};

const GOLD: Option<Kind> = Some(GOLD_4K);
const PISTON: Option<Kind> = Some(Kind::Vanilla("minecraft:piston"));
const IRON: Option<Kind> = Some(Kind::Vanilla("minecraft:iron_ingot"));

pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        GOLD,
        PISTON,
        GOLD,
        IRON,
        Some(ELECTRIC_MOTOR),
        IRON,
        GOLD,
        PISTON,
        GOLD,
    ],
    output: Stack {
        kind: BLOCK_PLACER,
        count: 1,
    },
    research: &RESEARCH,
};

pub(crate) fn lore(id: &str) -> Option<Tag> {
    (id == BLOCK_PLACER.id()).then(|| {
        Tag::List(vec![
            text("", "white", false),
            text("All Blocks in this Dispenser", "white", false),
            text("will automatically get placed", "white", false),
        ])
    })
}

static OWNER: AttachmentType<Option<u128>> = AttachmentType {
    namespace: "slimefun",
    name: "block_placer",
    default: || None,
    codec: Some(AttachmentCodec {
        encode: |owner| {
            let mut data = Compound::new();
            if let Some(owner) = owner {
                data.insert(
                    "owner".into(),
                    Tag::String(format!("{owner:032x}").as_str().into()),
                );
            }
            Ok(data)
        },
        decode: |data| decode_owner(data).ok_or(AttachmentError::Decode),
    }),
    copy_on_death: false,
    sync: false,
};

fn decode_owner(data: &Compound) -> Option<Option<u128>> {
    match data.get(&"owner".into()) {
        None => Some(None),
        Some(Tag::String(owner)) => Some(Some(parse_uuid(&owner.to_string().ok()?)?)),
        _ => None,
    }
}

pub(crate) fn marked(host: &dyn Host, entity: BlockEntityRef) -> bool {
    marker(host, entity).is_some()
}

fn marker(host: &dyn Host, entity: BlockEntityRef) -> Option<Option<u128>> {
    decode_owner(&host.block_entity_saved_attachment(entity, OWNER.namespace, OWNER.name)?)
}

pub(crate) struct Placement;

#[mod_api::horizon_part]
impl ItemPart for Placement {
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        if !BLOCK_PLACER.matches(host, event.stack) {
            return None;
        }
        let player = host.player(event.player)?;
        if research::can_use(host, player, &RESEARCH) {
            return None;
        }
        let _ = player.tell(
            host,
            Text::literal("Unlock the Block Placer with /sf research block_placer."),
        );
        Some(InteractionResult::Fail)
    }

    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        if let Some(player) = event.placer
            && BLOCK_PLACER.matches(host, event.stack)
        {
            let _ = OWNER.set(host, Holder::BlockEntity(event.pos), Some(player.uuid()));
        }
    }
}

fn block_is(host: &dyn Host, level: LevelRef, pos: Pos, id: &str) -> bool {
    level
        .block(host, pos)
        .is_some_and(|block| block.id(host) == id)
}

fn empty(host: &dyn Host, level: LevelRef, pos: Pos) -> Option<BlockState> {
    let state = level.block_state(host, pos)?;
    matches!(
        state.block().id(host).as_str(),
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
    .then_some(state)
}

fn below([x, y, z]: Pos) -> Pos {
    [x, y.saturating_sub(1), z]
}

const PAPER_EXCLUDED_TAGS: &[&str] = &[
    "minecraft:saplings",
    "minecraft:wooden_pressure_plates",
    "minecraft:pressure_plates",
    "minecraft:carpets",
    "minecraft:wool_carpets",
    "minecraft:beds",
    "minecraft:buttons",
    "minecraft:signs",
    "minecraft:doors",
    "minecraft:rails",
    "minecraft:small_flowers",
    "minecraft:coral_blocks",
    "minecraft:corals",
    "minecraft:banners",
];

const PAPER_EXCLUDED_BLOCKS: &[&str] = &[
    "torch",
    "redstone_torch",
    "soul_torch",
    "stone_pressure_plate",
    "light_weighted_pressure_plate",
    "heavy_weighted_pressure_plate",
    "lilac",
    "rose_bush",
    "peony",
    "tall_grass",
    "large_fern",
    "sunflower",
    "red_mushroom",
    "brown_mushroom",
    "crimson_fungus",
    "warped_fungus",
    "sugar_cane",
    "cactus",
    "bamboo",
    "vine",
    "ladder",
    "chorus_plant",
    "chorus_flower",
    "snow",
    "lever",
    "repeater",
    "tripwire_hook",
    "lily_pad",
    "dead_bush",
    "fern",
    "sea_pickle",
    "nether_wart",
    "seagrass",
    "tall_seagrass",
    "kelp",
    "bell",
    "lantern",
    "grass",
    "soul_lantern",
    "nether_sprouts",
    "crimson_roots",
    "twisting_vines",
    "warped_roots",
    "weeping_vines",
    "short_grass",
];

const PAPER_UNBREAKABLE: &[&str] = &[
    "command_block",
    "chain_command_block",
    "repeating_command_block",
    "bedrock",
    "barrier",
    "nether_portal",
    "end_portal",
    "end_portal_frame",
    "end_gateway",
    "structure_block",
    "structure_void",
    "jigsaw",
];

fn allowed_material(
    host: &dyn Host,
    level: LevelRef,
    target: Pos,
    material: &str,
    block: BlockKey,
    banned: Option<&[String]>,
) -> bool {
    let Some(vanilla) = material.strip_prefix("minecraft:") else {
        return true;
    };
    if vanilla == "cake" {
        let support = below(target);
        return level.block_state(host, support).is_some_and(|state| {
            level
                .block_shape(host, state, support, BlockShapeKind::Collision)
                .is_some_and(|boxes| !boxes.is_empty())
        });
    }
    if PAPER_EXCLUDED_BLOCKS.contains(&vanilla)
        || PAPER_EXCLUDED_TAGS
            .iter()
            .any(|tag| host.block_in_tag(block, tag) == Some(true))
    {
        return false;
    }
    !banned.map_or_else(
        || PAPER_UNBREAKABLE.contains(&vanilla),
        |blocks| {
            blocks.iter().any(|entry| {
                entry.eq_ignore_ascii_case(vanilla) || entry.eq_ignore_ascii_case(material)
            })
        },
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CustomBlock {
    OutputChest,
    Altar(Kind),
    ReinforcedSpawner,
    Network(String),
}

fn placeable_custom(id: &str) -> Option<CustomBlock> {
    if id == crate::spawner_repair::REINFORCED_SPAWNER.id() {
        return Some(CustomBlock::ReinforcedSpawner);
    }
    if id == OUTPUT_CHEST.id() {
        return Some(CustomBlock::OutputChest);
    }
    if let Some(kind) = crate::ancient_altar::paper_device(id) {
        return Some(CustomBlock::Altar(kind));
    }
    if !crate::ticker_policy::Tickers::current().allows(id) {
        return None;
    }
    let component = Component::for_item(id)?;
    if matches!(
        component,
        Component::EnergyRegulator
            | Component::CargoInput
            | Component::CargoOutput { .. }
            | Component::CargoConnector
            | Component::GpsTransmitter { .. }
    ) || matches!(
        id,
        "GEO_MINER"
            | "XP_COLLECTOR"
            | "GPS_TELEPORTATION_MATRIX"
            | "GPS_ACTIVATION_DEVICE_PERSONAL"
            | "DUCT_TAPE"
    ) {
        return None;
    }
    Some(CustomBlock::Network(id.to_owned()))
}

fn custom_name(stack: &ItemStack) -> Option<String> {
    let name = stack
        .components()?
        .as_nbt()
        .get(&"minecraft:custom_name".into())?;
    let literal = match name {
        Tag::String(value) => value,
        Tag::Compound(data) => match data.get(&"text".into())? {
            Tag::String(value) => value,
            _ => return None,
        },
        _ => return None,
    };
    literal.to_string().ok()
}

struct Attempt {
    level: LevelRef,
    source: Pos,
    carrier: BlockEntityRef,
    target: Pos,
    resource: ItemResource,
    placement: BlockState,
    custom: Option<CustomBlock>,
    name: Option<String>,
}

#[derive(Default)]
struct Queue {
    pending: BTreeMap<TaskId, Attempt>,
    ready: Vec<Attempt>,
}

#[derive(Default)]
pub(crate) struct Placers {
    queue: Mutex<Queue>,
    banned: Mutex<Option<Vec<String>>>,
}

impl Placers {
    pub(crate) fn configure(&self, banned: Option<Vec<String>>) {
        *self.banned.lock().unwrap() = banned;
    }

    pub(crate) fn clear(&self) {
        *self.queue.lock().unwrap() = Queue::default();
    }

    pub(crate) fn unload(&self, host: &dyn Host, dimension: &str) {
        let mut queue = self.queue.lock().unwrap();
        let still_open =
            |attempt: &Attempt| attempt.level.dimension().id(host).as_deref() != Some(dimension);
        queue.pending.retain(|_, attempt| still_open(attempt));
        queue.ready.retain(still_open);
    }

    pub(crate) fn selected(
        &self,
        host: &mut dyn ServerHost,
        event: &DispenserSelectedEvent<'_>,
    ) -> Flow {
        if crate::availability::use_denial(event.dimension, BLOCK_PLACER.id()).is_some() {
            return Flow::Continue;
        }
        let Some(level) = host.level(event.dimension) else {
            return Flow::Continue;
        };
        let Some(carrier) = host.block_entity(level, event.pos) else {
            return Flow::Continue;
        };
        let Some(owner) = marker(host, carrier) else {
            return Flow::Continue;
        };
        if block_is(host, level, below(event.pos), "minecraft:hopper") {
            return Flow::Continue;
        }
        if owner.is_some_and(|owner| !level.allow_device_placement(host, owner, event.target)) {
            return Flow::Cancel;
        }
        let material = event.stack.item().id(host);
        if material.starts_with("minecraft:") && material.ends_with("shulker_box") {
            return Flow::Continue;
        }
        if !event.viewed
            && empty(host, level, event.target).is_some()
            && let Some(block) = host.item_block(event.stack.item())
            && block.id(host) == material
            && allowed_material(
                host,
                level,
                event.target,
                &material,
                block,
                self.banned.lock().unwrap().as_deref(),
            )
            && let Some(placement) = host.block_state_default(block)
        {
            let custom = match item_id(event.stack) {
                Some(id) if crate::availability::use_denial(event.dimension, &id).is_some() => {
                    return Flow::Cancel;
                }
                Some(id) => match placeable_custom(&id) {
                    Some(CustomBlock::OutputChest)
                        if !crate::output_chest::enabled(event.dimension) =>
                    {
                        return Flow::Cancel;
                    }
                    Some(custom) => Some(custom),
                    None => return Flow::Cancel,
                },
                None => None,
            };
            if let Ok(task) = host.schedule_task(DELAY, 0) {
                self.queue.lock().unwrap().pending.insert(
                    task,
                    Attempt {
                        level,
                        source: event.pos,
                        carrier,
                        target: event.target,
                        resource: ItemResource::of(event.stack),
                        placement,
                        name: if custom.is_none() {
                            custom_name(event.stack)
                        } else {
                            None
                        },
                        custom,
                    },
                );
            }
        }
        Flow::Cancel
    }

    pub(crate) fn due(
        &self,
        host: &mut dyn ServerHost,
        event: &ScheduledTaskEvent<'_>,
        networks: &Mutex<Networks>,
    ) {
        if event.owner != "slimefun" {
            return;
        }
        let attempt = self.queue.lock().unwrap().pending.remove(&event.id);
        let Some(attempt) = attempt else {
            return;
        };
        if attempt.level.in_place(host) {
            attempt.place(host, networks);
        } else if host.block_entity(attempt.level, attempt.source) == Some(attempt.carrier) {
            self.queue.lock().unwrap().ready.push(attempt);
        }
    }

    pub(crate) fn tick(
        &self,
        host: &mut dyn ServerHost,
        level: LevelRef,
        networks: &Mutex<Networks>,
    ) {
        let work: Vec<_> = self
            .queue
            .lock()
            .unwrap()
            .ready
            .extract_if(.., |entry| entry.level == level)
            .collect();
        for attempt in work {
            attempt.place(host, networks);
        }
    }
}

impl Attempt {
    fn place(self, host: &mut dyn ServerHost, networks: &Mutex<Networks>) {
        if self.custom == Some(CustomBlock::OutputChest)
            && !crate::output_chest::enabled(host.dimension())
        {
            return;
        }
        let Some(expected) = empty(host, self.level, self.target) else {
            return;
        };
        if host.block_entity(self.level, self.source) != Some(self.carrier)
            || marker(host, self.carrier).is_none()
        {
            return;
        }
        // Paper emits the effect even when the matching material was removed during the delay.
        let _ = self
            .level
            .block_break_effect(host, self.target, self.placement);
        let Some(items) = self.level.capability(host, &ITEMS, self.source, None) else {
            return;
        };
        let (level, target) = (self.level, self.target);
        let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
        let _: Result<(), ()> = host.transaction(|host, tx| {
            // CraftInventory.removeItem matches the full resource in inventory order.
            if !(0..SLOTS.min(items.size(host))).any(|slot| {
                items.resource(host, slot).as_ref() == Some(&self.resource)
                    && items.extract(host, tx, slot, &self.resource, 1) == 1
            }) || host.block_entity(level, self.source) != Some(self.carrier)
                || level.block_state(host, target) != Some(expected)
                || level.set_block(host, target, self.placement, flags) != Ok(true)
            {
                return Err(());
            }
            let marked = match self.custom {
                Some(CustomBlock::OutputChest) => {
                    crate::output_chest::restore_paper_marker(host, level, target)
                }
                Some(CustomBlock::Altar(kind)) => crate::ancient_altar::restore_paper(
                    host,
                    level,
                    target,
                    kind,
                    &toml::Table::new(),
                )
                .is_ok(),
                Some(CustomBlock::ReinforcedSpawner) => crate::spawner_repair::place_repaired(
                    host,
                    level,
                    target,
                    self.resource.as_stack(),
                ),
                Some(CustomBlock::Network(ref id)) => networks
                    .lock()
                    .unwrap()
                    .restore(
                        host,
                        level,
                        target,
                        id,
                        &toml::Table::new(),
                        &toml::Table::new(),
                    )
                    .is_ok(),
                None => {
                    if let Some(name) = &self.name
                        && let Some(entity) = host.block_entity(level, target)
                    {
                        let _ = entity.set_name(host, Some(name));
                    }
                    true
                }
            };
            if marked {
                return Ok(());
            }
            if level.block_state(host, target) == Some(self.placement) {
                let _ = level.set_block(host, target, expected, flags);
            }
            Err(())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_placer_marker_preserves_the_owner_and_distinguishes_legacy_from_bad_data() {
        let codec = OWNER.codec.as_ref().unwrap();
        let owner = 0x4756f182_1f24_418f_96b1_2761dcd2edb0;
        assert_eq!(
            (codec.decode)(&(codec.encode)(&Some(owner)).unwrap()),
            Ok(Some(owner))
        );
        assert_eq!((codec.decode)(&Compound::new()), Ok(None));
        let data = Compound::from([("owner".into(), Tag::String("broken".into()))]);
        assert!(matches!(
            (codec.decode)(&data),
            Err(AttachmentError::Decode)
        ));
        let paper = Compound::from([(
            "owner".into(),
            Tag::String("4756f182-1f24-418f-96b1-2761dcd2edb0".into()),
        )]);
        assert_eq!(decode_owner(&paper), Some(Some(owner)));
    }

    #[test]
    fn only_paper_placeable_custom_machines_can_be_installed_by_the_placer() {
        assert_eq!(placeable_custom("BLOCK_PLACER"), None);
        assert_eq!(placeable_custom("CARGO_NODE_INPUT"), None);
        assert_eq!(placeable_custom("CARGO_NODE_OUTPUT"), None);
        assert_eq!(placeable_custom("ENERGY_REGULATOR"), None);
        assert_eq!(placeable_custom("DUCT_TAPE"), None);
        assert_eq!(placeable_custom("BROKEN_SPAWNER"), None);
        assert_eq!(
            placeable_custom("REINFORCED_SPAWNER"),
            Some(CustomBlock::ReinforcedSpawner)
        );
        assert_eq!(
            placeable_custom("ELECTRIC_FURNACE"),
            Some(CustomBlock::Network("ELECTRIC_FURNACE".into()))
        );
        assert_eq!(
            placeable_custom("ANCIENT_PEDESTAL"),
            Some(CustomBlock::Altar(crate::ancient_altar::ANCIENT_PEDESTAL))
        );
        assert_eq!(CRAFTING.inputs[4], Some(ELECTRIC_MOTOR));
        assert_eq!(CRAFTING.research.cost, 17);
    }
}

pub(crate) fn restore_drop(level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>) {
    let Some(entity) = event.block_entity else {
        return;
    };
    if level
        .block_entity_saved_attachment(entity, OWNER.namespace, OWNER.name)
        .and_then(|data| decode_owner(&data))
        .is_none()
    {
        return;
    }
    if let Some(key) = level.item_key_by_name(BLOCK_PLACER.material())
        && let Some(drop) = event.drops.iter_mut().find(|drop| drop.stack.item() == key)
    {
        BLOCK_PLACER.decorate(&mut drop.stack);
    }
}

pub(crate) fn restore_paper(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Pos,
    row: &toml::Table,
) -> Result<String, String> {
    let metadata = row
        .get("metadata")
        .and_then(toml::Value::as_table)
        .ok_or("Missing Paper Block Placer metadata.")?;
    let owner = metadata
        .get("owner")
        .map(|value| {
            value
                .as_str()
                .and_then(parse_uuid)
                .ok_or("Invalid Paper Block Placer owner UUID.")
        })
        .transpose()?;
    if !level.in_place(host) || !block_is(host, level, pos, BLOCK_PLACER.material()) {
        return Err("Load the imported Block Placer's dispenser in the active dimension.".into());
    }
    let carrier = host
        .block_entity(level, pos)
        .ok_or("The imported Block Placer has no live dispenser inventory.")?;
    if marker(host, carrier).is_some_and(|existing| existing != owner) {
        return Err("The live Block Placer already has a different owner.".into());
    }
    let archived = if row.contains_key("paper_inventory") {
        storage::validate_inventory_preset(row, BLOCK_PLACER.id())?;
        storage::decode_archived_slots(host, row, SLOTS as u32)?
    } else {
        Default::default()
    };
    let items = level
        .capability(host, &ITEMS, pos, None)
        .ok_or("The imported dispenser has no item slots.")?;
    if items.size(host) != SLOTS {
        return Err("Unexpected Block Placer inventory size.".into());
    }
    host.transaction(|host, tx| {
        for (slot, stack) in archived {
            let slot = slot as usize;
            let resource = ItemResource::of(&stack);
            match items.resource(host, slot) {
                Some(live) if live == resource && items.amount(host, slot) == stack.count() => {}
                None if items.insert(host, tx, slot, &resource, stack.count()) == stack.count() => {
                }
                _ => {
                    return Err(
                        "A live dispenser slot differs from the archived Block Placer inventory."
                            .into(),
                    );
                }
            }
        }
        OWNER
            .set(host, Holder::BlockEntity(pos), owner)
            .map_err(|_| "The dispenser cannot store its Block Placer owner.")?;
        Ok("Restored the Slimefun Block Placer.".into())
    })
}
