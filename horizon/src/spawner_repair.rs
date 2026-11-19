// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::BTreeMap,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockDropsEvent, BlockKey, BreakBlockEvent,
    Compound, Flow, Hand, Holder, Host, HostSide, HostTransaction, InteractionResult, ItemKey,
    ItemPart, ItemPlace, ItemStack, LevelHost, LevelRef, OnBlockPlacedByEvent,
    PlayerDestroyedBlockEvent, PlayerRef, RandomSource, Registrar, ServerHost, Tag, Text,
    UseOnEvent,
};

use crate::{
    armor_forge::ANCIENT_RUNE_ENDER,
    backpacks::ESSENCE_OF_AFTERLIFE,
    factory::experience_collector::FILLED_FLASK_OF_KNOWLEDGE,
    items::{FERROSILICON, GILDED_IRON, item, item_id, text},
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

const CARRIER: &str = "minecraft:spawner";
const BLOCK_DATA: &str = "minecraft:block_entity_data";

item!(
    PICKAXE_OF_CONTAINMENT,
    "iron_pickaxe",
    "Pickaxe of Containment",
    "red",
    None,
    None
);
item!(
    BROKEN_SPAWNER,
    "spawner",
    "Broken Spawner",
    "red",
    None,
    None
);
item!(
    REINFORCED_SPAWNER,
    "spawner",
    "Reinforced Spawner",
    "aqua",
    None,
    None
);

pub(crate) const CONTAINMENT: Research = Research {
    id: "pickaxe_of_containment",
    name: "Pickaxe of Containment",
    cost: 14,
};
pub(crate) const REPAIRING: Research = Research {
    id: "repaired_spawner",
    name: "Repairing Spawners",
    cost: 15,
};

pub(crate) const TOOL: CraftingRecipe = CraftingRecipe {
    inputs: [
        Some(FERROSILICON),
        Some(FERROSILICON),
        Some(FERROSILICON),
        None,
        Some(GILDED_IRON),
        None,
        None,
        Some(GILDED_IRON),
        None,
    ],
    output: Stack {
        kind: PICKAXE_OF_CONTAINMENT,
        count: 1,
    },
    research: &CONTAINMENT,
};

pub(crate) const ALTAR: CraftingRecipe = CraftingRecipe {
    inputs: [
        Some(ANCIENT_RUNE_ENDER),
        Some(FILLED_FLASK_OF_KNOWLEDGE),
        Some(ESSENCE_OF_AFTERLIFE),
        Some(FILLED_FLASK_OF_KNOWLEDGE),
        Some(BROKEN_SPAWNER),
        Some(FILLED_FLASK_OF_KNOWLEDGE),
        Some(ESSENCE_OF_AFTERLIFE),
        Some(FILLED_FLASK_OF_KNOWLEDGE),
        Some(ANCIENT_RUNE_ENDER),
    ],
    output: Stack {
        kind: REINFORCED_SPAWNER,
        count: 1,
    },
    research: &REPAIRING,
};

fn mob_label(id: &str) -> String {
    let species = id.strip_prefix("minecraft:").unwrap_or(id);
    species
        .split('_')
        .filter_map(|word| {
            let mut chars = word.chars();
            let first = chars.next()?;
            Some(first.to_uppercase().chain(chars).collect::<String>())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn spawner_lore(label: &str, broken: bool) -> Tag {
    let mut lines = vec![Tag::compound([
        ("text", Tag::String("Type: ".into())),
        ("color", Tag::String("gray".into())),
        ("italic", Tag::Byte(0)),
        ("extra", Tag::List(vec![text(label, "aqua", false)])),
    ])];
    if broken {
        lines.extend([
            Tag::String("".into()),
            text(
                "Fractured, must be repaired in an Ancient Altar",
                "red",
                false,
            ),
        ]);
    }
    Tag::List(lines)
}

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    let lore = match id {
        "PICKAXE_OF_CONTAINMENT" => Tag::List(vec![
            Tag::String("".into()),
            text("Can pickup Spawners", "blue", false),
        ]),
        "BROKEN_SPAWNER" => spawner_lore("<Type>", true),
        "REINFORCED_SPAWNER" => spawner_lore("<Type>", false),
        _ => return,
    };
    components.insert("minecraft:lore".into(), lore);
}

fn text_value(node: &Tag, literal: &mut String) {
    match node {
        Tag::String(value) => literal.push_str(&value.to_string_lossy()),
        Tag::List(nodes) => {
            for child in nodes {
                text_value(child, literal);
            }
        }
        Tag::Compound(_) => {
            if let Some(value) = node.get("text") {
                text_value(value, literal);
            }
            if let Some(extra) = node.get("extra") {
                text_value(extra, literal);
            }
        }
        _ => {}
    }
}

fn parse_lore_type(line: &Tag) -> Option<String> {
    let mut text = String::new();
    text_value(line, &mut text);
    let mut plain = String::new();
    let mut letters = text.chars();
    while let Some(letter) = letters.next() {
        if letter == '§' {
            let _ = letters.next();
        } else {
            plain.push(letter);
        }
    }
    let name = plain.strip_prefix("Type: ")?.trim();
    if name.is_empty() || name.contains('<') {
        return None;
    }
    let name = name.to_ascii_lowercase().replace(' ', "_");
    let path = match name.as_str() {
        "mushroom_cow" => "mooshroom",
        "snowman" => "snow_golem",
        "pig_zombie" => "zombified_piglin",
        _ => &name,
    };
    Some(format!("minecraft:{path}"))
}

fn known_species(host: &dyn Host, species: String) -> Option<String> {
    let id = mod_api::Identifier::parse(&species).ok()?;
    if id.namespace() != "minecraft" {
        return None;
    }
    host.registry_entry("minecraft:entity_type", &species)?;
    Some(species)
}

fn type_from_block_tag(data: &Compound) -> Option<String> {
    let Tag::String(id) = data.get(&"SpawnData".into())?.get("entity")?.get("id")? else {
        return None;
    };
    let id = id.to_string().ok()?;
    Some(if id.contains(':') {
        id
    } else {
        format!("minecraft:{id}")
    })
}

fn item_species(host: &dyn Host, stack: &ItemStack) -> Option<String> {
    let components = stack.components()?.as_nbt();
    if let Some(Tag::List(lines)) = components.get(&"minecraft:lore".into())
        && let Some(species) = lines.iter().find_map(parse_lore_type)
    {
        return known_species(host, species);
    }
    let Tag::Compound(payload) = components.get(&BLOCK_DATA.into())? else {
        return None;
    };
    known_species(host, type_from_block_tag(payload)?)
}

fn spawn_data(species: &str) -> Tag {
    Tag::compound([(
        "entity",
        Tag::compound([("id", Tag::String(species.into()))]),
    )])
}

fn spawner_item(host: &dyn Host, species: &str, repaired: bool) -> Option<ItemStack> {
    let kind = if repaired {
        REINFORCED_SPAWNER
    } else {
        BROKEN_SPAWNER
    };
    let mut stack = kind.stack(host, 1)?;
    stack
        .apply_components(&Compound::from([
            (
                "minecraft:lore".into(),
                spawner_lore(&mob_label(species), !repaired),
            ),
            (
                BLOCK_DATA.into(),
                Tag::compound([
                    ("id", Tag::String("minecraft:mob_spawner".into())),
                    ("SpawnData", spawn_data(species)),
                ]),
            ),
        ]))
        .ok()?;
    Some(stack)
}

/// The ritual's output carries the original Bukkit entity kind.
pub(crate) fn repair(host: &dyn Host, catalyst: &ItemStack) -> Option<ItemStack> {
    let species = item_species(host, catalyst).unwrap_or_else(|| "minecraft:pig".to_owned());
    spawner_item(host, &species, true)
}

static REPAIRED: AttachmentType<bool> = AttachmentType {
    namespace: "slimefun",
    name: "reinforced_spawner",
    default: || false,
    codec: Some(AttachmentCodec {
        encode: |repaired| {
            Ok(Compound::from([(
                "repaired".into(),
                Tag::Byte(i8::from(*repaired)),
            )]))
        },
        decode: |data| match data.get(&"repaired".into()) {
            Some(Tag::Byte(1)) => Ok(true),
            Some(Tag::Byte(0)) => Ok(false),
            _ => Err(AttachmentError::Decode),
        },
    }),
    copy_on_death: false,
    sync: false,
};

fn marker_is_repaired(data: Option<Compound>) -> bool {
    data.as_ref().and_then(|data| data.get(&"repaired".into())) == Some(&Tag::Byte(1))
}

pub(crate) fn place_repaired(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    stack: &ItemStack,
) -> bool {
    if !REINFORCED_SPAWNER.matches(host, stack)
        || !level
            .block(host, pos)
            .is_some_and(|block| block.id(host) == CARRIER)
    {
        return false;
    }
    let Some(entity) = host.block_entity(level, pos) else {
        return false;
    };
    let species = item_species(host, stack);
    host.transaction(|host, _| {
        if let Some(species) = species
            && entity
                .data(host)
                .and_then(|data| type_from_block_tag(&data))
                .as_deref()
                != Some(species.as_str())
            && entity.merge_data(
                host,
                Compound::from([("SpawnData".into(), spawn_data(&species))]),
            ) != Ok(true)
        {
            return Err(());
        }
        REPAIRED
            .set(host, Holder::BlockEntity(pos), true)
            .map_err(|_| ())?;
        Ok(())
    })
    .is_ok()
}

/// Reattach the Paper identity to the migrated vanilla spawner in place.
pub(crate) fn restore_paper(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
) -> Result<String, String> {
    if !level.in_place(host) || !level.is_loaded(host, pos) {
        return Err("Load the Paper spawner in the active dimension first.".into());
    }
    if !level
        .block(host, pos)
        .is_some_and(|block| block.id(host) == CARRIER)
    {
        return Err("The archived Reinforced Spawner needs its existing spawner block.".into());
    }
    let entity = host
        .block_entity(level, pos)
        .ok_or("The existing spawner data is unavailable.")?;
    if !marker_is_repaired(host.block_entity_saved_attachment(
        entity,
        REPAIRED.namespace,
        REPAIRED.name,
    )) {
        REPAIRED
            .set(host, Holder::BlockEntity(pos), true)
            .map_err(|error| format!("Could not save the Paper spawner identity: {error:?}"))?;
    }
    Ok("Restored REINFORCED_SPAWNER without replacing its native configuration.".into())
}

struct Placement;

#[mod_api::horizon_part]
impl ItemPart for Placement {
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        if BROKEN_SPAWNER.matches(host, event.stack) {
            return Some(InteractionResult::Fail);
        }
        if REINFORCED_SPAWNER.matches(host, event.stack) {
            let player = host.player(event.player)?;
            if !research::can_use(host, player, &REPAIRING) {
                let _ = player.tell(
                    host,
                    Text::literal("Unlock Repairing Spawners with /sf research repaired_spawner."),
                );
                return Some(InteractionResult::Fail);
            }
        }
        None
    }

    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        if let Some(level) = event.placer.and_then(|player| player.entity().level(host)) {
            let _ = place_repaired(host, level, event.pos, event.stack);
        }
    }
}

struct RemovedCandidate {
    level: LevelRef,
    pos: [i32; 3],
    repaired: bool,
}

pub(crate) struct Spawners {
    block: OnceLock<BlockKey>,
    tool: OnceLock<ItemKey>,
    pending: Mutex<BTreeMap<u128, RemovedCandidate>>,
    allow_eggs: AtomicBool,
}

impl Default for Spawners {
    fn default() -> Self {
        Self {
            block: OnceLock::new(),
            tool: OnceLock::new(),
            pending: Mutex::new(BTreeMap::new()),
            allow_eggs: AtomicBool::new(true),
        }
    }
}

fn holding_pickaxe(host: &mut dyn ServerHost, player: PlayerRef) -> bool {
    ItemPlace::PlayerHand {
        player: player.uuid(),
        hand: Hand::Main,
    }
    .stack(host)
    .is_some_and(|stack| PICKAXE_OF_CONTAINMENT.matches(host, &stack))
}

impl Spawners {
    pub(crate) fn register(&self, registrar: &mut dyn Registrar) {
        let _ = self.block.set(registrar.block_key(CARRIER));
        let _ = self
            .tool
            .set(registrar.item_key(PICKAXE_OF_CONTAINMENT.material()));
        let carrier = registrar.item_key(CARRIER);
        registrar.item_part(carrier, Placement);
    }

    pub(crate) fn configure(&self, allow_eggs: bool) {
        self.allow_eggs.store(allow_eggs, Ordering::Relaxed);
    }

    pub(crate) fn prevents_egg_use(
        &self,
        host: &dyn Host,
        level: LevelRef,
        pos: [i32; 3],
        stack: &ItemStack,
    ) -> bool {
        !self.allow_eggs.load(Ordering::Relaxed)
            && stack.item().id(host).ends_with("_spawn_egg")
            && host.block_entity(level, pos).is_some_and(|entity| {
                marker_is_repaired(host.block_entity_saved_attachment(
                    entity,
                    REPAIRED.namespace,
                    REPAIRED.name,
                ))
            })
    }

    pub(crate) fn prepare(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        event: &mut BreakBlockEvent<'_>,
    ) -> Flow {
        self.forget(player.uuid());
        if player.is_fake(host) == Some(true) || !holding_pickaxe(host, player) {
            return Flow::Continue;
        }
        if !research::can_use(host, player, &CONTAINMENT) {
            let _ = player.tell(
                host,
                Text::literal(
                    "Unlock the Pickaxe of Containment with /sf research pickaxe_of_containment.",
                ),
            );
            return Flow::Cancel;
        }
        if Some(&event.state.block()) == self.block.get()
            && let Some(entity) = host.block_entity(level, event.pos)
            && entity
                .data(host)
                .and_then(|data| type_from_block_tag(&data))
                .and_then(|id| known_species(host, id))
                .is_some()
        {
            let repaired = marker_is_repaired(host.block_entity_saved_attachment(
                entity,
                REPAIRED.namespace,
                REPAIRED.name,
            ));
            self.pending.lock().unwrap().insert(
                player.uuid(),
                RemovedCandidate {
                    level,
                    pos: event.pos,
                    repaired,
                },
            );
            *event.exp_to_drop = Some(0);
        }
        Flow::Continue
    }

    pub(crate) fn destroyed(&self, host: &mut dyn ServerHost, event: &PlayerDestroyedBlockEvent) {
        let Some(player) = host.players().acting_player() else {
            return;
        };
        let candidate = self.pending.lock().unwrap().remove(&player.uuid());
        let Some(candidate) = candidate else {
            return;
        };
        if candidate.pos != event.pos
            || Some(&event.state.block()) != self.block.get()
            || player.entity().level(host) != Some(candidate.level)
            || !holding_pickaxe(host, player)
        {
            return;
        }
        let Some(species) = event
            .block_entity
            .as_ref()
            .and_then(type_from_block_tag)
            .and_then(|id| known_species(host, id))
        else {
            return;
        };
        let Some(stack) = spawner_item(host, &species, candidate.repaired) else {
            return;
        };
        let level = candidate.level;
        let mut xyz = event.pos.map(f64::from);
        if let Some(mut random) = level.random(host) {
            // CraftWorld.dropItemNaturally samples once per axis around the block origin.
            xyz = xyz.map(|p| p + (random.next_double() * 0.5 - 0.25));
            let _ = level.set_random(host, random);
        }
        xyz[1] -= 0.125; // Half the vanilla item entity's height.
        let _ = level.spawn_item(host, xyz[0], xyz[1], xyz[2], stack, 10);
    }

    pub(crate) fn suppress_base_drops(
        &self,
        level: &dyn LevelHost,
        event: &mut BlockDropsEvent<'_>,
    ) {
        if Some(&event.state.block()) != self.block.get() || event.breaker.is_none() {
            return;
        }
        let containment = event.tool.is_some_and(|tool| {
            Some(&tool.item()) == self.tool.get()
                && item_id(tool).as_deref() == Some(PICKAXE_OF_CONTAINMENT.id())
        });
        let repaired = event.block_entity.is_some_and(|entity| {
            marker_is_repaired(level.block_entity_saved_attachment(
                entity,
                REPAIRED.namespace,
                REPAIRED.name,
            ))
        });
        if containment || repaired {
            event.drops.clear();
        }
    }

    pub(crate) fn forget(&self, player: u128) {
        self.pending.lock().unwrap().remove(&player);
    }

    pub(crate) fn clear(&self) {
        self.pending.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn paper_containment_pickaxe_and_spawner_repair_recipe_keep_original_ids() {
        assert_eq!(TOOL.research.id, "pickaxe_of_containment");
        assert_eq!(TOOL.research.cost, 14);
        assert_eq!(
            TOOL.inputs.map(|kind| kind.map_or("", |kind| kind.id())),
            [
                "FERROSILICON",
                "FERROSILICON",
                "FERROSILICON",
                "",
                "GILDED_IRON",
                "",
                "",
                "GILDED_IRON",
                ""
            ],
        );
        assert_eq!(ALTAR.research.cost, 15);
        assert_eq!(
            ALTAR.inputs.map(|kind| kind.map_or("", |kind| kind.id())),
            [
                "ANCIENT_RUNE_ENDER",
                "FILLED_FLASK_OF_KNOWLEDGE",
                "ESSENCE_OF_AFTERLIFE",
                "FILLED_FLASK_OF_KNOWLEDGE",
                "BROKEN_SPAWNER",
                "FILLED_FLASK_OF_KNOWLEDGE",
                "ESSENCE_OF_AFTERLIFE",
                "FILLED_FLASK_OF_KNOWLEDGE",
                "ANCIENT_RUNE_ENDER",
            ],
        );
        assert_eq!(ALTAR.output.kind.id(), "REINFORCED_SPAWNER");
    }

    #[test]
    fn the_native_spawner_component_and_migrated_paper_lore_preserve_entity_type() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse(CARRIER).unwrap()])
            .unwrap();
        let broken = spawner_item(&host, "minecraft:cave_spider", false).unwrap();
        let repaired = spawner_item(&host, "minecraft:cave_spider", true).unwrap();
        for (stack, marker) in [(&broken, BROKEN_SPAWNER), (&repaired, REINFORCED_SPAWNER)] {
            assert!(marker.matches(&host, stack));
            let components = stack.components().unwrap().as_nbt();
            let Tag::Compound(data) = components.get(&BLOCK_DATA.into()).unwrap() else {
                panic!("expected a spawner data component");
            };
            assert_eq!(
                type_from_block_tag(data).as_deref(),
                Some("minecraft:cave_spider")
            );
            let Tag::List(lore) = components.get(&"minecraft:lore".into()).unwrap() else {
                panic!("expected Paper spawner lore");
            };
            assert_eq!(
                parse_lore_type(&lore[0]).as_deref(),
                Some("minecraft:cave_spider")
            );
        }
        assert_eq!(
            parse_lore_type(&Tag::String("§7Type: §bMushroom Cow".into())).as_deref(),
            Some("minecraft:mooshroom"),
        );
        assert!(parse_lore_type(&Tag::String("Type: <Type>".into())).is_none());
    }
}
